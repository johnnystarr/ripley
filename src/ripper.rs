use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info};

use crate::metadata::DiscMetadata;

#[derive(Debug, Clone)]
pub struct RipProgress {
    pub current_track: u32,
    pub total_tracks: u32,
    pub track_name: String,
    pub percentage: f32,
    pub status: RipStatus,
    pub speed_mbps: Option<f32>, // Ripping speed in MB/s
    pub bytes_processed: Option<u64>, // Total bytes processed so far
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum RipStatus {
    Idle,
    FetchingMetadata,
    Ripping,
    Encoding,
    Complete,
    Error(String),
}

/// Rip a CD using abcde
pub async fn rip_cd<F, L>(
    device: &str,
    metadata: &DiscMetadata,
    output_dir: &Path,
    quality: u8,
    mut progress_callback: F,
    mut log_callback: L,
) -> Result<()>
where
    F: FnMut(RipProgress) + Send,
    L: FnMut(String) + Send,
{
    info!("Starting rip of {} - {} from {}", metadata.artist, metadata.album, device);

    // Kill any existing abcde processes for this device
    info!("Checking for existing abcde processes on {}...", device);
    match Command::new("pkill")
        .arg("-f")
        .arg(format!("abcde.*{}", device))
        .output()
        .await {
            Ok(_) => info!("Killed any existing abcde processes"),
            Err(e) => tracing::error!("Failed to kill abcde processes: {}", e),
        }
    
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Unmount the disc with retries (macOS may auto-remount)
    info!("Unmounting {}...", device);
    for attempt in 1..=3 {
        match Command::new("diskutil")
            .arg("unmountDisk")
            .arg("force")
            .arg(device)
            .output()
            .await {
            Ok(output) if output.status.success() => {
                info!("Successfully unmounted {} (attempt {})", device, attempt);
                break;
            }
            Ok(output) => {
                let err = String::from_utf8_lossy(&output.stderr);
                tracing::error!("Unmount attempt {} failed: {}", attempt, err);
                if attempt < 3 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
            Err(e) => {
                tracing::error!("Could not execute unmount command: {}", e);
            }
        }
    }
    
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    // Create output directory structure: Artist/Album/
    info!("Creating output directory...");
    let album_dir = output_dir
        .join(sanitize_filename(&metadata.artist))
        .join(sanitize_filename(&metadata.album));
    
    if let Err(e) = tokio::fs::create_dir_all(&album_dir).await {
        tracing::error!("Failed to create album directory {}: {}", album_dir.display(), e);
        return Err(anyhow::anyhow!("Failed to create album directory: {}", e));
    }
    info!("Output directory: {}", album_dir.display());

    // Configure abcde
    info!("Generating abcde config...");
    let config = create_abcde_config(&album_dir, quality)?;
    let config_path = album_dir.join(".abcde.conf");
    if let Err(e) = tokio::fs::write(&config_path, &config).await {
        tracing::error!("Failed to write abcde config: {}", e);
        return Err(anyhow::anyhow!("Failed to write config: {}", e));
    }
    info!("Config written to: {}", config_path.display());

    // Run abcde with progress tracking
    info!("Starting abcde process...");
    let mut child = match Command::new("abcde")
        .arg("-c")
        .arg(&config_path)
        .arg("-d")
        .arg(device)
        .arg("-o")
        .arg("flac")
        .arg("-N")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to spawn abcde: {}", e);
                return Err(anyhow::anyhow!("Failed to start abcde: {}", e));
            }
        };
    info!("abcde process started, monitoring output...");

    // Track progress by parsing abcde output in real-time
    let stdout = child.stdout.take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
    let stderr = child.stderr.take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let total_tracks = metadata.tracks.len() as u32;
    let mut current_track = 0;
    let mut stderr_lines = Vec::new();

    // Process output in real-time
    loop {
        tokio::select! {
            result = stdout_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        info!("abcde: {}", line);
                        log_callback(line.clone());
                        
                        // Parse progress from abcde output
                        if line.contains("Grabbing track") || line.contains("Reading track") {
                            if let Some(track_num) = line.split("track").nth(1)
                                .and_then(|s| s.split_whitespace().next())
                                .and_then(|s| s.parse::<u32>().ok()) 
                            {
                                current_track = track_num;
                                let track_name = metadata.tracks.get((current_track - 1) as usize)
                                    .map(|t| t.title.clone())
                                    .unwrap_or_else(|| format!("Track {}", current_track));

                                progress_callback(RipProgress {
                                    current_track,
                                    total_tracks,
                                    track_name: track_name.clone(),
                                    percentage: (current_track as f32 / total_tracks as f32) * 100.0,
                                    status: RipStatus::Ripping,
                                    speed_mbps: None,
                                    bytes_processed: None,
                                });
                            }
                        } else if line.contains("Encoding") || line.contains("encoding") {
                            progress_callback(RipProgress {
                                current_track,
                                total_tracks,
                                track_name: metadata.tracks.get((current_track - 1) as usize)
                                    .map(|t| t.title.clone())
                                    .unwrap_or_else(|| format!("Track {}", current_track)),
                                percentage: (current_track as f32 / total_tracks as f32) * 100.0,
                                status: RipStatus::Encoding,
                                speed_mbps: None,
                                bytes_processed: None,
                            });
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        debug!("Error reading stdout: {}", e);
                        break;
                    }
                }
            }
            result = stderr_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        if line.contains("ERROR") || line.contains("error") || line.contains("failed") {
                            tracing::error!("abcde: {}", line);
                            log_callback(format!("ERROR: {}", line));
                        } else {
                            tracing::warn!("abcde: {}", line);
                            log_callback(line.clone());
                        }
                        stderr_lines.push(line);
                    }
                    Ok(None) => {}
                    Err(e) => debug!("Error reading stderr: {}", e),
                }
            }
        }
    }

    // Drain any remaining stderr
    while let Ok(Some(line)) = stderr_reader.next_line().await {
        if line.contains("ERROR") || line.contains("error") || line.contains("failed") {
            tracing::error!("abcde: {}", line);
        } else {
            tracing::warn!("abcde: {}", line);
        }
        stderr_lines.push(line);
    }

    // Wait for completion
    let status = child.wait().await?;

    if status.success() {
        info!("Successfully ripped {} - {}", metadata.artist, metadata.album);
        progress_callback(RipProgress {
            current_track: total_tracks,
            total_tracks,
            track_name: "Complete".to_string(),
            percentage: 100.0,
            status: RipStatus::Complete,
            speed_mbps: None,
            bytes_processed: None,
        });
        Ok(())
    } else {
        // Include stderr in error message
        let error_msg = if !stderr_lines.is_empty() {
            format!("abcde failed with status: {}\nErrors:\n{}", 
                status, 
                stderr_lines.join("\n"))
        } else {
            format!("abcde failed with status: {}", status)
        };
        tracing::error!("{}", error_msg);
        Err(anyhow::anyhow!(error_msg))
    }
}

/// Create minimal abcde configuration
pub fn create_abcde_config(output_dir: &Path, quality: u8) -> Result<String> {
    let config = format!(
        r#"
# Ripley auto-generated abcde config
OUTPUTDIR="{}"
OUTPUTTYPE="flac"
FLACOPTS="-{}f"
INTERACTIVE=n
PADTRACKS=y
OUTPUTFORMAT='${{TRACKNUM}}. ${{TRACKFILE}}'
VAOUTPUTFORMAT='${{TRACKNUM}}. ${{ARTISTFILE}}-${{TRACKFILE}}'
ONETRACKOUTPUTFORMAT='${{ARTISTFILE}}-${{ALBUMFILE}}'
MAXPROCS=2
CDDBMETHOD=cddb
CDDBURL="http://gnudb.gnudb.org/~cddb/cddb.cgi"
"#,
        output_dir.display(),
        quality
    );

    Ok(config)
}

/// Convert string to PascalCase with periods (e.g., "Foster's Home for Imaginary Friends" -> "Fosters.Home.For.Imaginary.Friends")
pub fn to_pascal_case_with_periods(name: &str) -> String {
    // First, remove apostrophes to handle "Foster's" -> "Fosters"
    let cleaned = name.replace('\'', "");
    
    cleaned
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase())
                }
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// Sanitize filename by removing invalid characters
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case_with_periods() {
        assert_eq!(to_pascal_case_with_periods("Foster's Home for Imaginary Friends"), "Fosters.Home.For.Imaginary.Friends");
        assert_eq!(to_pascal_case_with_periods("The Matrix"), "The.Matrix");
        assert_eq!(to_pascal_case_with_periods("AC/DC - Back in Black"), "Ac.Dc.Back.In.Black");
        assert_eq!(to_pascal_case_with_periods("Star Wars: Episode IV"), "Star.Wars.Episode.Iv");
        assert_eq!(to_pascal_case_with_periods("21 Jump Street"), "21.Jump.Street");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Normal Name"), "Normal Name");
        assert_eq!(sanitize_filename("Name/With/Slashes"), "Name_With_Slashes");
        assert_eq!(sanitize_filename("Name:With:Colons"), "Name_With_Colons");
        assert_eq!(sanitize_filename("Name*?<>|"), "Name_____");
        assert_eq!(sanitize_filename("AC/DC"), "AC_DC");
    }

    #[test]
    fn test_create_abcde_config() {
        let output = Path::new("/tmp/test");
        
        let config = create_abcde_config(output, 8).unwrap();
        assert!(config.contains("OUTPUTTYPE=\"flac\""));
        assert!(config.contains("FLACOPTS=\"-8f\""));
        assert!(config.contains("INTERACTIVE=n"));
        
        let config = create_abcde_config(output, 0).unwrap();
        assert!(config.contains("FLACOPTS=\"-0f\""));
    }

    #[test]
    fn test_rip_progress() {
        let progress = RipProgress {
            current_track: 1,
            total_tracks: 10,
            track_name: "Test".to_string(),
            percentage: 10.0,
            status: RipStatus::Ripping,
            speed_mbps: None,
            bytes_processed: None,
        };
        
        assert_eq!(progress.current_track, 1);
        assert_eq!(progress.status, RipStatus::Ripping);
    }
}
