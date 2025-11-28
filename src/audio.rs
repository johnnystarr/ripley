use anyhow::{Context, Result};
#[cfg(feature = "audio")]
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use tracing::{debug, warn};

/// Get the path to audio files directory
pub fn get_sounds_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("ripley").join("sounds")
}

/// Play an audio notification
pub async fn play_notification(sound_name: &str) -> Result<()> {
    let sounds_dir = get_sounds_dir();
    let sound_path = sounds_dir.join(format!("{}.mp3", sound_name));

    if !sound_path.exists() {
        warn!("Sound file not found: {}", sound_path.display());
        return Ok(()); // Don't fail if sound file is missing
    }

    debug!("Playing notification: {}", sound_path.display());

    // Spawn a blocking task to play audio
    tokio::task::spawn_blocking(move || {
        if let Err(e) = play_audio_sync(&sound_path) {
            warn!("Failed to play audio: {}", e);
        }
    });

    Ok(())
}

/// Play audio file synchronously (blocking)
#[cfg(feature = "audio")]
fn play_audio_sync(path: &PathBuf) -> Result<()> {
    let (_stream, stream_handle) = OutputStream::try_default()
        .context("Failed to open audio output stream")?;
    
    let sink = Sink::try_new(&stream_handle)
        .context("Failed to create audio sink")?;

    let file = File::open(path)
        .context("Failed to open audio file")?;
    
    let source = Decoder::new(BufReader::new(file))
        .context("Failed to decode audio file")?;

    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}

#[cfg(not(feature = "audio"))]
fn play_audio_sync(_path: &PathBuf) -> Result<()> {
    // Audio feature disabled - no-op
    Ok(())
}

/// Initialize the sounds directory and create README
pub async fn initialize_sounds_dir() -> Result<()> {
    let sounds_dir = get_sounds_dir();
    
    if !sounds_dir.exists() {
        tokio::fs::create_dir_all(&sounds_dir).await
            .context("Failed to create sounds directory")?;
        
        // Create a README
        let readme = r#"Ripley Audio Notifications
===========================

Place your audio notification files here:

- complete.mp3: Played when a CD finishes ripping successfully
- error.mp3: Played when metadata lookup fails after all retries

These files are optional. If not present, notifications will be skipped.
"#;
        
        let readme_path = sounds_dir.join("README.txt");
        tokio::fs::write(readme_path, readme).await?;
    }

    Ok(())
}
