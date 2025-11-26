use anyhow::{Context, Result};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MediaType {
    AudioCD,
    DVD,
    BluRay,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DriveInfo {
    pub device: String,
    pub name: String,
    pub has_audio_cd: bool, // Kept for backward compatibility
    pub media_type: MediaType,
}

/// Detect all CD/DVD drives on macOS
pub async fn detect_drives() -> Result<Vec<DriveInfo>> {
    let output = Command::new("diskutil")
        .arg("list")
        .output()
        .context("Failed to execute diskutil")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut drives = Vec::new();

    // Parse diskutil output to find optical drives
    for line in stdout.lines() {
        if line.contains("/dev/disk") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(device) = parts.first() {
                let device = device.trim();
                if is_optical_drive(device).await? {
                    let media_type = detect_media_type(device).await?;
                    let has_audio = matches!(media_type, MediaType::AudioCD);
                    drives.push(DriveInfo {
                        device: device.to_string(),
                        name: format!("Drive {}", device),
                        has_audio_cd: has_audio,
                        media_type,
                    });
                }
            }
        }
    }

    // Also check drutil for more accurate optical drive detection
    let drutil_output = Command::new("drutil")
        .arg("status")
        .output();

    if let Ok(output) = drutil_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("Type: CD-ROM") || stdout.contains("Type: Audio") {
            debug!("Found optical drive via drutil");
        }
    }

    Ok(drives)
}

/// Check if a device is an optical drive
async fn is_optical_drive(device: &str) -> Result<bool> {
    let output = Command::new("diskutil")
        .arg("info")
        .arg(device)
        .output()
        .context("Failed to get disk info")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("CD-ROM") || 
       stdout.contains("DVD") || 
       stdout.contains("Optical"))
}

/// Detect what type of media is in the optical drive
async fn detect_media_type(device: &str) -> Result<MediaType> {
    // Use drutil to check media type
    let output = Command::new("drutil")
        .arg("status")
        .arg("-drive")
        .arg(device)
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // No media present
        if stdout.contains("No Media") || stdout.contains("not present") {
            return Ok(MediaType::None);
        }
        
        // Check for audio CD
        if stdout.contains("Audio") || stdout.contains("CDDA") {
            return Ok(MediaType::AudioCD);
        }
        
        // Check for Blu-ray (check before DVD as some output might contain both)
        if stdout.contains("Blu-ray") || stdout.contains("BD") || stdout.contains("BDROM") {
            return Ok(MediaType::BluRay);
        }
        
        // Check for DVD (video or data)
        if stdout.contains("DVD") {
            return Ok(MediaType::DVD);
        }
    }

    // Fallback: use diskutil to check
    let output = Command::new("diskutil")
        .arg("info")
        .arg(device)
        .output()
        .context("Failed to check media type")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    if stdout.contains("Audio") || stdout.contains("CDDA") {
        return Ok(MediaType::AudioCD);
    }
    
    // Check for Blu-ray before DVD
    if stdout.contains("Blu-ray") || stdout.contains("BD-") || stdout.contains("BDROM") {
        return Ok(MediaType::BluRay);
    }
    
    if stdout.contains("DVD") {
        return Ok(MediaType::DVD);
    }
    
    Ok(MediaType::None)
}

/// Continuously monitor for new drives and disc insertions
pub async fn monitor_drives<F>(mut callback: F) -> Result<()>
where
    F: FnMut(Vec<DriveInfo>) + Send + 'static,
{
    let mut previous_drives: Vec<DriveInfo> = Vec::new();

    loop {
        let current_drives = detect_drives().await?;

        // Check for changes
        if current_drives != previous_drives {
            info!("Drive state changed: {} drives detected", current_drives.len());
            for drive in &current_drives {
                if drive.has_audio_cd {
                    info!("Audio CD detected in {}", drive.device);
                }
            }
            callback(current_drives.clone());
            previous_drives = current_drives;
        }

        // Poll every 2 seconds
        sleep(Duration::from_secs(2)).await;
    }
}

/// Eject a disc from the specified drive
pub async fn eject_disc(device: &str) -> Result<()> {
    info!("Ejecting disc from {}", device);
    
    let output = Command::new("drutil")
        .arg("eject")
        .arg(device)
        .output()
        .context("Failed to eject disc")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        warn!("Failed to eject disc: {}", err);
        return Err(anyhow::anyhow!("Eject failed: {}", err));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drive_info_creation() {
        let drive = DriveInfo {
            device: "/dev/disk2".to_string(),
            name: "Drive /dev/disk2".to_string(),
            has_audio_cd: true,
            media_type: MediaType::AudioCD,
        };
        
        assert_eq!(drive.device, "/dev/disk2");
        assert!(drive.has_audio_cd);
        assert_eq!(drive.media_type, MediaType::AudioCD);
    }

    #[test]
    fn test_media_type_variants() {
        assert_eq!(MediaType::AudioCD, MediaType::AudioCD);
        assert_eq!(MediaType::DVD, MediaType::DVD);
        assert_eq!(MediaType::BluRay, MediaType::BluRay);
        assert_eq!(MediaType::None, MediaType::None);
        assert_ne!(MediaType::AudioCD, MediaType::DVD);
        assert_ne!(MediaType::DVD, MediaType::BluRay);
    }

    #[test]
    fn test_drive_info_equality() {
        let drive1 = DriveInfo {
            device: "/dev/disk2".to_string(),
            name: "Drive 1".to_string(),
            has_audio_cd: true,
            media_type: MediaType::AudioCD,
        };
        
        let drive2 = DriveInfo {
            device: "/dev/disk2".to_string(),
            name: "Drive 1".to_string(),
            has_audio_cd: true,
            media_type: MediaType::AudioCD,
        };
        
        assert_eq!(drive1, drive2);
    }

    #[test]
    fn test_drive_info_no_audio() {
        let drive = DriveInfo {
            device: "/dev/disk2".to_string(),
            name: "Empty Drive".to_string(),
            has_audio_cd: false,
            media_type: MediaType::None,
        };
        
        assert!(!drive.has_audio_cd);
        assert_eq!(drive.media_type, MediaType::None);
    }
    
    #[test]
    fn test_drive_info_dvd() {
        let drive = DriveInfo {
            device: "/dev/disk3".to_string(),
            name: "DVD Drive".to_string(),
            has_audio_cd: false,
            media_type: MediaType::DVD,
        };
        
        assert!(!drive.has_audio_cd);
        assert_eq!(drive.media_type, MediaType::DVD);
    }
}
