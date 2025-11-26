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
    // Use drutil to check media type (without device parameter - drutil doesn't support /dev/disk# syntax)
    // drutil status works for the primary optical drive
    let output = Command::new("drutil")
        .arg("status")
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("drutil status output: {}", stdout);
        
        // Check if this drutil output corresponds to our device by checking the Name field
        let device_matches = stdout.lines()
            .any(|line| line.contains("Name:") && line.contains(device));
        
        if !device_matches {
            debug!("drutil output doesn't match device {}, falling back to diskutil", device);
        } else {
            // No media present
            if stdout.contains("No Media") || stdout.contains("not present") {
                return Ok(MediaType::None);
            }
            
            // Check for audio CD by looking at the Type field
            if stdout.contains("Type: CD-ROM") || stdout.contains("Type: Audio") || stdout.contains("CDDA") {
                info!("Detected Audio CD in {} (via drutil)", device);
                return Ok(MediaType::AudioCD);
            }
            
            // Check for DVD by looking at the Type field (not drive model which may contain "BD")
            if stdout.contains("Type: DVD") {
                info!("Detected DVD in {} (via drutil)", device);
                return Ok(MediaType::DVD);
            }
            
            // Check for Blu-ray by looking at the Type field
            if stdout.contains("Type: BD") || stdout.contains("Type: Blu-ray") {
                info!("Detected Blu-ray in {} (via drutil)", device);
                return Ok(MediaType::BluRay);
            }
        }
    }

    // Fallback: use diskutil to check file system
    let output = Command::new("diskutil")
        .arg("info")
        .arg(device)
        .output()
        .context("Failed to check media type")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    debug!("diskutil info output for {}: {}", device, stdout);
    
    // Try to determine type from file system
    // Note: diskutil "Device / Media Name" often shows drive model (which may contain "BD-RE")
    // so we need to be very careful and use more reliable indicators
    
    // Check if it's mounted and what file system it has
    let is_audio_cd = stdout.contains("Type (Bundle):             cda") 
        || stdout.contains("Audio CD");
    
    if is_audio_cd {
        info!("Detected Audio CD in {} (via diskutil)", device);
        return Ok(MediaType::AudioCD);
    }
    
    // For video discs, check the file system
    // UDF is used by both DVDs and Blu-rays, so we can't reliably distinguish
    // If drutil didn't work, we have to make an educated guess or return None
    let has_udf = stdout.contains("UDF") || stdout.contains("Universal Disk Format");
    
    if has_udf {
        // Could be DVD or Blu-ray - check for BDMV directory would require mounting
        // For now, default to DVD since that's more common
        warn!("Detected UDF disc in {}, assuming DVD (drutil detection failed)", device);
        return Ok(MediaType::DVD);
    }
    
    warn!("Could not determine media type for {}", device);
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
