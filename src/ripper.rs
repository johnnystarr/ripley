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

    // Create output directory structure: Artist/Album/
    let album_dir = output_dir
        .join(sanitize_filename(&metadata.artist))
        .join(sanitize_filename(&metadata.album));
    
    tokio::fs::create_dir_all(&album_dir).await
        .context("Failed to create album directory")?;

    // Configure abcde
    let config = create_abcde_config(&album_dir, quality)?;
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

    // Track progress by parsing abcde output
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();

    let total_tracks = metadata.tracks.len() as u32;
    let mut current_track = 0;

    while let Ok(Some(line)) = reader.next_line().await {
        debug!("abcde: {}", line);

        // Parse progress from abcde output
        if line.contains("Grabbing track") {
            current_track += 1;
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
        } else if line.contains("Encoding") {
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
        Err(anyhow::anyhow!("abcde failed with status: {}", status))
    }
}

/// Create abcde configuration
fn create_abcde_config(output_dir: &Path, quality: u8) -> Result<String> {
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
CDDBMETHOD=musicbrainz
"#,
        output_dir.display(),
        quality
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
