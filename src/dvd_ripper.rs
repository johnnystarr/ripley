use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::dvd_metadata::{DvdMetadata, MediaType};
use crate::ripper::RipProgress;

/// Parse duration string "H:MM:SS" or "HH:MM:SS" to minutes
fn parse_duration_to_minutes(duration: &str) -> Option<u32> {
    let parts: Vec<&str> = duration.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    
    let hours: u32 = parts[0].parse().ok()?;
    let minutes: u32 = parts[1].parse().ok()?;
    let seconds: u32 = parts[2].parse().ok()?;
    
    Some(hours * 60 + minutes + if seconds >= 30 { 1 } else { 0 })
}

/// Rip a DVD using makemkvcon
pub async fn rip_dvd<F, L, E>(
    device: &str,
    output_dir: &Path,
    metadata: Option<&DvdMetadata>,
    mut progress_callback: F,
    mut log_callback: L,
    mut episode_callback: E,
) -> Result<()>
where
    F: FnMut(RipProgress) + Send,
    L: FnMut(String) + Send,
    E: for<'a> FnMut(&'a Path, u32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> + Send,
{
    info!("Starting DVD rip from {}", device);
    log_callback("Starting DVD rip...".to_string());

    // Check if makemkvcon is installed
    let check = Command::new("which")
        .arg("makemkvcon")
        .output()
        .await?;
    
    if !check.status.success() {
        return Err(anyhow!("makemkvcon not found. Install MakeMKV from https://www.makemkv.com/"));
    }

    // Setup MakeMKV settings to skip subtitles
    setup_makemkv_settings().await?;

    // Create output directory
    if let Err(e) = tokio::fs::create_dir_all(output_dir).await {
        tracing::error!("Failed to create output directory {}: {}", output_dir.display(), e);
        return Err(anyhow!("Failed to create output directory: {}", e));
    }

    info!("Output directory: {}", output_dir.display());
    log_callback(format!("Output: {}", output_dir.display()));

    // First, scan the disc to get info
    log_callback("Scanning DVD...".to_string());
    info!("Scanning DVD in {}", device);
    
    progress_callback(RipProgress {
        current_track: 0,
        total_tracks: 1,
        track_name: "Scanning...".to_string(),
        percentage: 0.0,
        status: crate::ripper::RipStatus::FetchingMetadata,
        speed_mbps: None,
        bytes_processed: None,
    });

    let mut scan_child = Command::new("makemkvcon")
        .arg("-r")
        .arg("info")
        .arg(format!("dev:{}", device))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("Failed to start makemkvcon: {}", e))?;

    let stdout = scan_child.stdout.take()
        .ok_or_else(|| anyhow!("Failed to capture stdout"))?;
    let stderr = scan_child.stderr.take()
        .ok_or_else(|| anyhow!("Failed to capture stderr"))?;
    
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();
    let mut title_count = 0;
    let mut title_durations: Vec<(usize, String)> = Vec::new(); // (title_index, duration)

    // Parse scan output
    loop {
        tokio::select! {
            result = stdout_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        debug!("makemkvcon scan: {}", line);
                        
                        // Count titles
                        if line.starts_with("TCOUNT:") {
                            if let Some(count_str) = line.strip_prefix("TCOUNT:") {
                                title_count = count_str.trim().parse().unwrap_or(0);
                                info!("Found {} titles on DVD", title_count);
                                log_callback(format!("Found {} titles", title_count));
                            }
                        }
                        
                        // Parse title durations: TINFO:0,9,0,"2:59:37"
                        if line.starts_with("TINFO:") && line.contains(",9,0,") {
                            let parts: Vec<&str> = line.split(',').collect();
                            if parts.len() >= 4 {
                                if let Some(title_idx) = parts[0].strip_prefix("TINFO:").and_then(|s| s.parse::<usize>().ok()) {
                                    let duration = parts[3].trim_matches('"').to_string();
                                    title_durations.push((title_idx, duration.clone()));
                                    info!("Title {}: {}", title_idx, duration);
                                }
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        debug!("Error reading scan stdout: {}", e);
                        break;
                    }
                }
            }
            result = stderr_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        tracing::warn!("makemkvcon: {}", line);
                        log_callback(format!("WARN: {}", line));
                    }
                    Ok(None) => {}
                    Err(e) => debug!("Error reading scan stderr: {}", e),
                }
            }
        }
    }

    let scan_status = scan_child.wait().await?;
    if !scan_status.success() {
        return Err(anyhow!("DVD scan failed with status: {}", scan_status));
    }

    if title_count == 0 {
        return Err(anyhow!("No titles found on DVD"));
    }
    
    // Match episodes to disc titles by duration if we have metadata
    let _metadata = if let Some(meta) = metadata {
        if meta.media_type == MediaType::TVShow && !meta.episodes.is_empty() {
            log_callback("Matching episodes to disc titles by duration...".to_string());
            let matched_episodes = crate::dvd_metadata::match_episodes_by_duration(
                meta.episodes.clone(),
                &title_durations
            );
            let mut updated_meta = meta.clone();
            updated_meta.episodes = matched_episodes;
            Some(updated_meta)
        } else {
            Some(meta.clone())
        }
    } else {
        None
    };

    // Filter titles to rip based on duration (skip "Play All" compilations)
    let titles_to_rip: Vec<u32> = title_durations.iter()
        .filter_map(|(idx, duration)| {
            if let Some(minutes) = parse_duration_to_minutes(duration) {
                // Only rip titles that look like individual episodes (18-50 min)
                // Skip compilations/play-all (typically 90-200 min for multi-episode discs)
                // For movies, we expect them to be clearly movie-length (> 70 min but handled separately)
                #[allow(clippy::manual_range_contains)]
                if minutes >= 18 && minutes <= 70 {
                    info!("Will rip title {} ({} min)", idx, minutes);
                    Some(*idx as u32)
                } else {
                    log_callback(format!("⏭️  Skipping title {} ({} min) - outside episode range", idx, minutes));
                    info!("Skipping title {} ({} min) - likely compilation or play-all", idx, minutes);
                    None
                }
            } else {
                None
            }
        })
        .collect();

    log_callback(format!("Ripping {} titles (filtered from {})", titles_to_rip.len(), title_count));
    info!("Starting DVD rip of titles: {:?}", titles_to_rip);
    
    progress_callback(RipProgress {
        current_track: 0,
        total_tracks: titles_to_rip.len() as u32,
        track_name: "Starting rip...".to_string(),
        percentage: 0.0,
        status: crate::ripper::RipStatus::Ripping,
        speed_mbps: None,
        bytes_processed: None,
    });

    // Rip each title individually
    for (idx, title_num) in titles_to_rip.iter().enumerate() {
        let title_progress = (idx as f32 / titles_to_rip.len() as f32) * 100.0;
        log_callback(format!("📀 Starting title {} ({}/{})", title_num, idx + 1, titles_to_rip.len()));
        
        progress_callback(RipProgress {
            current_track: idx as u32,
            total_tracks: titles_to_rip.len() as u32,
            track_name: format!("Title {} - 0%", title_num),
            percentage: title_progress,
            speed_mbps: None,
            bytes_processed: None,
            status: crate::ripper::RipStatus::Ripping,
        });
        
        let mut rip_child = Command::new("makemkvcon")
            .arg("-r")
            .arg("--progress=-same")  // Output progress to stdout
            .arg("--minlength=300")   // Minimum title length in seconds (5 minutes)
            .arg("--noscan")          // Don't scan disc again, we already did
            .arg("mkv")
            .arg(format!("dev:{}", device))
            .arg(title_num.to_string())  // Rip specific title by number
            .arg(output_dir)
            .env("MAKEMKV_PROFILE", "default")  // Use default profile
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Failed to start makemkvcon rip: {}", e))?;

        let stdout = rip_child.stdout.take()
            .ok_or_else(|| anyhow!("Failed to capture stdout"))?;
        let stderr = rip_child.stderr.take()
            .ok_or_else(|| anyhow!("Failed to capture stderr"))?;
        
        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();
        #[allow(unused_assignments)]
        let mut title_percentage = 0.0;

        // Parse rip output for this title
        loop {
            tokio::select! {
                result = stdout_reader.next_line() => {
                    match result {
                        Ok(Some(line)) => {
                            debug!("makemkvcon: {}", line);
                            
                            // Parse progress: "PRGV:current,total,max"
                            if line.starts_with("PRGV:") {
                                if let Some(progress_str) = line.strip_prefix("PRGV:") {
                                    let parts: Vec<&str> = progress_str.split(',').collect();
                                    if parts.len() >= 3 {
                                        if let (Ok(current), Ok(max)) = (parts[0].parse::<u32>(), parts[2].parse::<u32>()) {
                                            title_percentage = if max > 0 {
                                                (current as f32 / max as f32) * 100.0
                                            } else {
                                                0.0
                                            };
                                            
                                            // Calculate overall progress
                                            let overall = ((idx as f32 + title_percentage / 100.0) / titles_to_rip.len() as f32) * 100.0;
                                            
                                            progress_callback(RipProgress {
                                                current_track: idx as u32,
                                                total_tracks: titles_to_rip.len() as u32,
                                                track_name: format!("Title {} - {:.0}%", title_num, title_percentage),
                                                percentage: overall,
                                                status: crate::ripper::RipStatus::Ripping,
                                                speed_mbps: None,
                                                bytes_processed: None,
                                            });
                                        }
                                    }
                                }
                            }
                            
                            // Log important messages
                            if line.starts_with("MSG:") || line.starts_with("TINFO:") {
                                log_callback(line.clone());
                            }
                        }
                            Ok(None) => break,
                        Err(e) => {
                            debug!("Error reading rip stdout: {}", e);
                            break;
                        }
                    }
                }
                result = stderr_reader.next_line() => {
                    match result {
                        Ok(Some(line)) => {
                            if line.contains("ERROR") || line.contains("error") || line.contains("failed") {
                                tracing::error!("makemkvcon: {}", line);
                                log_callback(format!("❌ ERROR: {}", line));
                            } else {
                                debug!("makemkvcon stderr: {}", line);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => debug!("Error reading rip stderr: {}", e),
                    }
                }
            }
        }

        let rip_status = rip_child.wait().await?;
        
        if !rip_status.success() {
            return Err(anyhow!("Failed to rip title {}: status {}", title_num, rip_status));
        }
        
        log_callback(format!("✅ Title {} ripped", title_num));
        
        // Find the ripped file for this title
        let mut ripped_file: Option<std::path::PathBuf> = None;
        if let Ok(mut entries) = tokio::fs::read_dir(output_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("mkv") {
                    // Check if this file was just created (within last 10 seconds)
                    if let Ok(file_metadata) = tokio::fs::metadata(&path).await {
                        if let Ok(modified) = file_metadata.modified() {
                            let age = std::time::SystemTime::now()
                                .duration_since(modified)
                                .unwrap_or_default();
                            if age.as_secs() < 10 {
                                ripped_file = Some(path);
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        // Process this episode immediately: OpenAI -> Validate -> Move to completed
        if let Some(file_path) = ripped_file {
            // Don't log here - let the callback handle logging to avoid duplicates
            let file_path_for_callback = file_path.clone();
            if let Err(e) = episode_callback(&file_path_for_callback, *title_num).await {
                warn!("Failed to process episode {}: {}", title_num, e);
                log_callback(format!("⚠️  Could not process title {}: {}", title_num, e));
            } else {
                log_callback(format!("✅ Title {} processed and moved to completed", title_num));
            }
        } else {
            warn!("Could not find ripped file for title {}", title_num);
            log_callback(format!("⚠️  Could not find file for title {}", title_num));
        }
    }
    
    info!("Successfully ripped and renamed all titles");
    log_callback("✅ DVD rip complete".to_string());
    
    progress_callback(RipProgress {
        current_track: titles_to_rip.len() as u32,
        total_tracks: titles_to_rip.len() as u32,
        track_name: "Complete".to_string(),
        percentage: 100.0,
        status: crate::ripper::RipStatus::Complete,
        speed_mbps: None,
        bytes_processed: None,
    });
    
    Ok(())
}


/// Rename MKV files based on metadata (batch mode, kept for compatibility)
#[allow(dead_code)]
async fn rename_dvd_files(output_dir: &Path, metadata: &DvdMetadata) -> Result<()> {
    use tokio::fs;
    
    let mut entries = fs::read_dir(output_dir).await?;
    let mut mkv_files = Vec::new();
    
    // Collect all MKV files
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("mkv") {
            mkv_files.push(path);
        }
    }
    
    // Sort files by name to ensure consistent ordering
    mkv_files.sort();
    
    // Simple naming: DISC_LABEL-TIMESTAMP.mkv for all files
    use std::time::{SystemTime, UNIX_EPOCH};
    let disc_label = metadata.title.replace(' ', "_").replace("/", "_");
    
    for (file_idx, file_path) in mkv_files.iter().enumerate() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Add file index if multiple files
        let new_name = if mkv_files.len() > 1 {
            format!("{}-{}-{:02}.mkv", disc_label, timestamp, file_idx + 1)
        } else {
            format!("{}-{}.mkv", disc_label, timestamp)
        };
        
        let new_path = output_dir.join(&new_name);
        
        info!("Renaming {} -> {}", file_path.display(), new_name);
        fs::rename(file_path, &new_path).await?;
        
        // Small delay to ensure different timestamps if processing quickly
        if mkv_files.len() > 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
    
    Ok(())
}

/// Configure MakeMKV to skip subtitles
async fn setup_makemkv_settings() -> Result<()> {
    use std::env;
    use std::path::PathBuf;

    // Get MakeMKV data directory
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let makemkv_dir = PathBuf::from(home).join(".MakeMKV");
    
    // Create directory if it doesn't exist
    tokio::fs::create_dir_all(&makemkv_dir).await?;
    
    let settings_file = makemkv_dir.join("settings.conf");
    
    // Settings to disable subtitle selection by default
    // app_DefaultSelectionString controls what tracks are selected
    // Format: +AUDIOTRACK,+VIDEOTRACK,-SUBTITLETRACK
    let settings = r#"# Ripley auto-generated MakeMKV settings
# Skip subtitles by default
app_DefaultSelectionString = "+sel:all,-sel:subtitle"

# Minimum title length (5 minutes = 300 seconds)
app_MinLength = "300"
"#;

    tokio::fs::write(&settings_file, settings).await?;
    
    info!("MakeMKV settings configured: skip subtitles, minimum 5 minutes");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_makemkv_check() {
        // Just verify the module compiles and has correct structure
        // Actual functionality requires makemkvcon to be installed
    }
}
