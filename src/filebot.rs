use anyhow::Result;
use std::path::Path;
use tracing::{debug, info, warn};

/// Rename video files using Filebot to match TheTVDB/TMDB database order
pub async fn rename_with_filebot(
    output_dir: &Path,
    show_title: &str,
    log_callback: impl Fn(String) + Send + 'static,
) -> Result<()> {
    info!("Running Filebot to rename files in {} for '{}'", output_dir.display(), show_title);
    
    log_callback(format!("🤖 Running Filebot to fix episode ordering for '{}'...", show_title));
    
    // Check if filebot is installed
    let check = tokio::process::Command::new("which")
        .arg("filebot")
        .output()
        .await?;
    
    if !check.status.success() {
        warn!("Filebot not found. Install with: brew install filebot");
        log_callback("⚠️  Filebot not installed. Install with: brew install filebot".to_string());
        return Err(anyhow::anyhow!("Filebot not installed"));
    }
    
    log_callback("📝 Filebot analyzing episodes and renaming to broadcast order...".to_string());
    
    // Build filebot command to rename files
    let mut cmd = tokio::process::Command::new("filebot");
    cmd.arg("-rename")
        .arg(output_dir)
        .arg("--db").arg("TheTVDB")  // Use TheTVDB (best for TV series)
        .arg("--q").arg(show_title)
        .arg("--order").arg("Airdate")  // Use broadcast order
        .arg("--format").arg("{n.space('.')}.S{s.pad(2)}E{e.pad(2)}.{t.space('.')}")  // Format: Show.Name.S01E01.Episode.Title
        .arg("-non-strict");  // Allow fuzzy matching
    
    debug!("Filebot command: {:?}", cmd);
    
    // Run filebot to rename files
    let output = cmd.output().await?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    debug!("Filebot stdout: {}", stdout);
    debug!("Filebot stderr: {}", stderr);
    
    // Check if filebot succeeded
    if !output.status.success() {
        warn!("Filebot failed with status: {}", output.status);
        log_callback("⚠️  Filebot failed to rename files".to_string());
        return Err(anyhow::anyhow!("Filebot exited with error: {}", output.status));
    }
    
    // Parse output to count renames - look for [MOVE] lines
    let mut rename_count = 0;
    for line in stdout.lines() {
        if line.starts_with("[MOVE]") && line.contains("from [") && line.contains("] to [") {
            rename_count += 1;
            // Extract filenames for logging
            if let Some(from_start) = line.find("from [") {
                if let Some(from_end) = line[from_start..].find("] to [") {
                    let old_path = &line[from_start + 6..from_start + from_end];
                    if let Some(old_name) = std::path::Path::new(old_path).file_name() {
                        log_callback(format!("  ✓ {}", old_name.to_string_lossy()));
                    }
                }
            }
        }
    }
    
    if rename_count > 0 {
        info!("Filebot renamed {} files successfully", rename_count);
        log_callback(format!("✅ Filebot renamed {} files to match database order", rename_count));
        Ok(())
    } else {
        warn!("Filebot didn't rename any files");
        log_callback("⚠️  Filebot couldn't match any files".to_string());
        Err(anyhow::anyhow!("No files were renamed"))
    }
}

/// Rename audio files using Filebot with MusicBrainz database
pub async fn rename_music_with_filebot(
    album_dir: &Path,
    log_callback: impl Fn(String) + Send + 'static,
) -> Result<()> {
    info!("Running Filebot to standardize music filenames in {}", album_dir.display());
    
    log_callback("🎵 Running Filebot to standardize track naming...".to_string());
    
    // Check if filebot is installed
    let check = tokio::process::Command::new("which")
        .arg("filebot")
        .output()
        .await?;
    
    if !check.status.success() {
        warn!("Filebot not found. Install with: brew install filebot");
        log_callback("⚠️  Filebot not installed (optional). Install with: brew install filebot".to_string());
        return Ok(()); // Not an error - Filebot is optional for music
    }
    
    log_callback("📝 Filebot analyzing album metadata from MusicBrainz...".to_string());
    
    // Build filebot command for music mode
    // Format: Artist/Album/01 - Track Title.flac
    let mut cmd = tokio::process::Command::new("filebot");
    cmd.arg("-rename")
        .arg(album_dir)
        .arg("--db").arg("AcoustID")  // Use AcoustID/MusicBrainz for music
        .arg("--format").arg("{artist}/{album}/{pi.pad(2)} - {t}")
        .arg("-non-strict");
    
    debug!("Filebot music command: {:?}", cmd);
    
    // Run filebot
    let output = cmd.output().await?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    debug!("Filebot stdout: {}", stdout);
    debug!("Filebot stderr: {}", stderr);
    
    // Parse output to count renamed files
    let mut rename_count = 0;
    for line in stdout.lines().chain(stderr.lines()) {
        if line.contains("[MOVE]") || line.contains("[RENAME]") {
            rename_count += 1;
            log_callback(format!("  {}", line));
        } else if line.contains("Processed") {
            log_callback(format!("  {}", line));
        }
    }
    
    if rename_count > 0 {
        info!("Filebot renamed {} music files successfully", rename_count);
        log_callback(format!("✅ Filebot standardized {} track names", rename_count));
    } else {
        log_callback("ℹ️  Filebot: Files already well-named".to_string());
    }
    
    Ok(())
}
