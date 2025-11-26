use anyhow::{Context, Result};
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
pub async fn rip_cd<F>(
    device: &str,
    metadata: &DiscMetadata,
    output_dir: &Path,
    quality: u8,
    mut progress_callback: F,
) -> Result<()>
where
    F: FnMut(RipProgress) + Send,
{
    info!("Starting rip of {} - {} from {}", metadata.artist, metadata.album, device);

    // Kill any existing abcde processes for this device
    info!("Checking for existing abcde processes on {}...", device);
    let _ = Command::new("pkill")
        .arg("-f")
        .arg(&format!("abcde.*{}", device))
        .output()
        .await;
    
    // Give processes time to die
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Unmount the disc if it's mounted (required for abcde to access it)
    info!("Unmounting {} if mounted...", device);
    let unmount_result = Command::new("diskutil")
        .arg("unmountDisk")
        .arg(device)
        .output()
        .await;
    
    match unmount_result {
        Ok(output) if output.status.success() => {
            info!("Successfully unmounted {}", device);
        }
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr);
            debug!("Unmount message: {}", err);
        }
        Err(e) => {
            debug!("Could not unmount {}: {}", device, e);
        }
    }
    
    // Give the system a moment to release the device
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create output directory structure: Artist/Album/
    let album_dir = output_dir
        .join(sanitize_filename(&metadata.artist))
        .join(sanitize_filename(&metadata.album));
    
    tokio::fs::create_dir_all(&album_dir).await
        .context("Failed to create album directory")?;

    // Configure abcde with metadata
    let config = create_abcde_config(&album_dir, quality, metadata)?;
    let config_path = album_dir.join(".abcde.conf");
    tokio::fs::write(&config_path, config).await?;

    // Run abcde with progress tracking
    let mut child = Command::new("abcde")
        .arg("-c")
        .arg(&config_path)
        .arg("-d")
        .arg(device)
        .arg("-o")
        .arg("flac")
        .arg("-N")  // Non-interactive
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to start abcde - is it installed?")?;

    // Track progress by parsing abcde output in real-time
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
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
                        
                        // Parse progress from abcde output
                        if line.contains("Grabbing track") || line.contains("Reading track") {
                            if let Some(track_num) = line.split("track").nth(1)
                                .and_then(|s| s.trim().split_whitespace().next())
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
                        } else {
                            tracing::warn!("abcde: {}", line);
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

/// Create abcde configuration with metadata
fn create_abcde_config(output_dir: &Path, quality: u8, metadata: &DiscMetadata) -> Result<String> {
    // Build track names for abcde
    let mut track_data = String::new();
    for (i, track) in metadata.tracks.iter().enumerate() {
        let track_num = i + 1;
        let safe_title = track.title.replace("'", "'\\''");
        track_data.push_str(&format!("TRACKNAME[{}]='{}'\n", track_num, safe_title));
    }
    
    let safe_artist = metadata.artist.replace("'", "'\\''");
    let safe_album = metadata.album.replace("'", "'\\''");
    
    let config = format!(
        r#"
# Ripley auto-generated abcde config
CDROM=/dev/cdrom
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

# Metadata from MusicBrainz
DARTIST='{}'
DALBUM='{}'
{}
"#,
        output_dir.display(),
        quality,
        safe_artist,
        safe_album,
        track_data
    );

    Ok(config)
}

/// Sanitize filename by removing invalid characters
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}
