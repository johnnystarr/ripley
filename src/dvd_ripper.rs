use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info};

use crate::ripper::RipProgress;

/// Rip a DVD using makemkvcon
pub async fn rip_dvd<F, L>(
    device: &str,
    output_dir: &Path,
    mut progress_callback: F,
    mut log_callback: L,
) -> Result<()>
where
    F: FnMut(RipProgress) + Send,
    L: FnMut(String) + Send,
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

    // Parse scan output
    loop {
        tokio::select! {
            result = stdout_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        debug!("makemkvcon scan: {}", line);
                        log_callback(line.clone());
                        
                        // Count titles
                        if line.starts_with("TCOUNT:") {
                            if let Some(count_str) = line.strip_prefix("TCOUNT:") {
                                title_count = count_str.trim().parse().unwrap_or(0);
                                info!("Found {} titles on DVD", title_count);
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

    log_callback(format!("Found {} titles, starting rip...", title_count));

    // Rip the disc (all titles)
    info!("Starting DVD rip");
    
    progress_callback(RipProgress {
        current_track: 0,
        total_tracks: title_count,
        track_name: "Starting rip...".to_string(),
        percentage: 0.0,
        status: crate::ripper::RipStatus::Ripping,
    });

    let mut rip_child = Command::new("makemkvcon")
        .arg("-r")
        .arg("mkv")
        .arg(format!("dev:{}", device))
        .arg("all")
        .arg(output_dir)
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
    let mut current_title = 0;

    // Parse rip output
    loop {
        tokio::select! {
            result = stdout_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        info!("makemkvcon: {}", line);
                        log_callback(line.clone());
                        
                        // Parse progress: "PRGV:current,total,max"
                        if line.starts_with("PRGV:") {
                            if let Some(progress_str) = line.strip_prefix("PRGV:") {
                                let parts: Vec<&str> = progress_str.split(',').collect();
                                if parts.len() >= 3 {
                                    if let (Ok(current), Ok(max)) = (parts[0].parse::<u32>(), parts[2].parse::<u32>()) {
                                        let percentage = if max > 0 {
                                            (current as f32 / max as f32) * 100.0
                                        } else {
                                            0.0
                                        };
                                        
                                        progress_callback(RipProgress {
                                            current_track: current_title,
                                            total_tracks: title_count,
                                            track_name: format!("Title {}", current_title),
                                            percentage,
                                            status: crate::ripper::RipStatus::Ripping,
                                        });
                                    }
                                }
                            }
                        }
                        
                        // Track title changes: "PRGC:current,total,message"
                        if line.starts_with("PRGC:") {
                            if let Some(progress_str) = line.strip_prefix("PRGC:") {
                                let parts: Vec<&str> = progress_str.split(',').collect();
                                if !parts.is_empty() {
                                    if let Ok(title) = parts[0].parse::<u32>() {
                                        current_title = title;
                                    }
                                }
                            }
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
                            log_callback(format!("ERROR: {}", line));
                        } else {
                            tracing::warn!("makemkvcon: {}", line);
                            log_callback(line.clone());
                        }
                    }
                    Ok(None) => {}
                    Err(e) => debug!("Error reading rip stderr: {}", e),
                }
            }
        }
    }

    let rip_status = rip_child.wait().await?;
    
    if rip_status.success() {
        info!("Successfully ripped DVD");
        log_callback("✅ DVD rip complete".to_string());
        
        progress_callback(RipProgress {
            current_track: title_count,
            total_tracks: title_count,
            track_name: "Complete".to_string(),
            percentage: 100.0,
            status: crate::ripper::RipStatus::Complete,
        });
        
        Ok(())
    } else {
        Err(anyhow!("DVD rip failed with status: {}", rip_status))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_makemkv_check() {
        // Just verify the module compiles and has correct structure
        // Actual functionality requires makemkvcon to be installed
    }
}
