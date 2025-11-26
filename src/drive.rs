use anyhow::{Context, Result};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DriveInfo {
    pub device: String,
    pub name: String,
    pub has_audio_cd: bool,
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
                    let has_audio = check_audio_cd(device).await?;
                    drives.push(DriveInfo {
                        device: device.to_string(),
                        name: format!("Drive {}", device),
                        has_audio_cd: has_audio,
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

/// Check if an optical drive contains an audio CD
async fn check_audio_cd(device: &str) -> Result<bool> {
    // Use drutil to check for audio CD
    let output = Command::new("drutil")
        .arg("status")
        .arg("-drive")
        .arg(device)
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("No Media") || stdout.contains("not present") {
            return Ok(false);
        }
        
        // Check if it's an audio CD
        if stdout.contains("Audio") || stdout.contains("CDDA") {
            return Ok(true);
        }
    }

    // Alternative: use diskutil to check for audio tracks
    let output = Command::new("diskutil")
        .arg("info")
        .arg(device)
        .output()
        .context("Failed to check for audio CD")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("Audio") || stdout.contains("CDDA"))
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
        };
        
        assert_eq!(drive.device, "/dev/disk2");
        assert!(drive.has_audio_cd);
    }

    #[test]
    fn test_drive_info_equality() {
        let drive1 = DriveInfo {
            device: "/dev/disk2".to_string(),
            name: "Drive 1".to_string(),
            has_audio_cd: true,
        };
        
        let drive2 = DriveInfo {
            device: "/dev/disk2".to_string(),
            name: "Drive 1".to_string(),
            has_audio_cd: true,
        };
        
        assert_eq!(drive1, drive2);
    }

    #[test]
    fn test_drive_info_no_audio() {
        let drive = DriveInfo {
            device: "/dev/disk2".to_string(),
            name: "Empty Drive".to_string(),
            has_audio_cd: false,
        };
        
        assert!(!drive.has_audio_cd);
    }
}
