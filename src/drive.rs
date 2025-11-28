use anyhow::{Context, Result};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MediaType {
    AudioCD,
    #[allow(clippy::upper_case_acronyms)]
    DVD,
    BluRay,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DriveInfo {
    pub device: String,
    pub name: String,
    pub has_audio_cd: bool, // Kept for backward compatibility
    pub media_type: MediaType,
}

/// Detect all CD/DVD drives (cross-platform: macOS and Linux)
pub async fn detect_drives() -> Result<Vec<DriveInfo>> {
    #[cfg(target_os = "macos")]
    {
        detect_drives_macos().await
    }
    
    #[cfg(target_os = "linux")]
    {
        detect_drives_linux().await
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        warn!("Unsupported platform - only macOS and Linux are supported");
        Ok(Vec::new())
    }
}

/// Detect drives on macOS using diskutil and drutil
#[cfg(target_os = "macos")]
async fn detect_drives_macos() -> Result<Vec<DriveInfo>> {
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

/// Detect drives on Linux using lsblk and udev
#[cfg(target_os = "linux")]
async fn detect_drives_linux() -> Result<Vec<DriveInfo>> {
    // Use lsblk to find optical drives (block devices with type "rom")
    let output = Command::new("lsblk")
        .arg("-n")
        .arg("-o")
        .arg("NAME,TYPE,MOUNTPOINT")
        .arg("-r")
        .output()
        .context("Failed to execute lsblk")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut drives = Vec::new();

    // Parse lsblk output to find optical drives (TYPE=rom)
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == "rom" {
            let device_name = parts[0];
            let device_path = format!("/dev/{}", device_name);
            
            // Check if it's actually an optical drive by looking at /sys/block
            if is_optical_drive(&device_path).await? {
                let media_type = detect_media_type(&device_path).await?;
                let has_audio = matches!(media_type, MediaType::AudioCD);
                
                // Try to get a friendly name from udev or use device name
                let name = get_drive_name_linux(&device_path).await
                    .unwrap_or_else(|_| format!("Drive {}", device_name));
                
                drives.push(DriveInfo {
                    device: device_path.clone(),
                    name,
                    has_audio_cd: has_audio,
                    media_type,
                });
            }
        }
    }

    Ok(drives)
}

/// Get friendly drive name on Linux using udev
#[cfg(target_os = "linux")]
async fn get_drive_name_linux(device: &str) -> Result<String> {
    // Try to get device model from udev or /sys
    if let Some(dev_name) = device.strip_prefix("/dev/") {
        // Try /sys/block/{dev}/device/model
        let model_path = format!("/sys/block/{}/device/model", dev_name);
        if let Ok(model) = std::fs::read_to_string(&model_path) {
            let model = model.trim();
            if !model.is_empty() {
                return Ok(format!("{} ({})", model, dev_name));
            }
        }
        
        // Fallback to udev
        if let Ok(output) = Command::new("udevadm")
            .arg("info")
            .arg("-q")
            .arg("name")
            .arg(device)
            .arg("--property")
            .arg("ID_MODEL")
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(model) = stdout.lines().find(|l| l.starts_with("ID_MODEL=")) {
                if let Some(model_value) = model.strip_prefix("ID_MODEL=") {
                    let model_clean = model_value.replace('_', " ");
                    if !model_clean.is_empty() {
                        return Ok(format!("{} ({})", model_clean, dev_name));
                    }
                }
            }
        }
    }
    
    Ok(format!("Drive {}", device))
}

/// Check if a device is an optical drive (cross-platform)
async fn is_optical_drive(device: &str) -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        is_optical_drive_macos(device).await
    }
    
    #[cfg(target_os = "linux")]
    {
        is_optical_drive_linux(device).await
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Ok(false)
    }
}

/// Check if a device is an optical drive on macOS
#[cfg(target_os = "macos")]
async fn is_optical_drive_macos(device: &str) -> Result<bool> {
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

/// Check if a device is an optical drive on Linux
#[cfg(target_os = "linux")]
async fn is_optical_drive_linux(device: &str) -> Result<bool> {
    // On Linux, check if device exists and is a block device with type "rom"
    if !device.starts_with("/dev/") {
        return Ok(false);
    }
    
    let dev_name = device.strip_prefix("/dev/").unwrap();
    
    // Check /sys/block/{dev}/queue/type - optical drives should exist here
    let sys_path = format!("/sys/block/{}", dev_name);
    if !std::path::Path::new(&sys_path).exists() {
        return Ok(false);
    }
    
    // Check if it's actually an optical drive by examining device type
    // Optical drives on Linux are typically /dev/sr* or /dev/cdrom*
    if dev_name.starts_with("sr") || dev_name.starts_with("cdrom") || dev_name == "cd" {
        return Ok(true);
    }
    
    // Additional check: try to read device type from sysfs
    let device_type_path = format!("/sys/block/{}/device/type", dev_name);
    if let Ok(device_type) = std::fs::read_to_string(&device_type_path) {
        // Type 5 is CD-ROM in Linux
        if device_type.trim() == "5" {
            return Ok(true);
        }
    }
    
    Ok(false)
}

/// Detect what type of media is in the optical drive (cross-platform)
async fn detect_media_type(device: &str) -> Result<MediaType> {
    #[cfg(target_os = "macos")]
    {
        detect_media_type_macos(device).await
    }
    
    #[cfg(target_os = "linux")]
    {
        detect_media_type_linux(device).await
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Ok(MediaType::None)
    }
}

/// Detect media type on macOS using drutil and diskutil
#[cfg(target_os = "macos")]
async fn detect_media_type_macos(device: &str) -> Result<MediaType> {
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
    // UDF is used by both DVDs and Blu-rays, so we need to distinguish
    let has_udf = stdout.contains("UDF") || stdout.contains("Universal Disk Format");
    
    if has_udf {
        // Try to check if this is a Blu-ray by checking for BDMV directory
        // First, check if disc is mounted
        let mount_point = stdout.lines()
            .find(|line| line.trim().starts_with("Mount Point:"))
            .and_then(|line| line.split(':').nth(1))
            .map(|s| s.trim().to_string());
        
        if let Some(mount) = mount_point {
            // Check for BDMV directory which indicates Blu-ray
            let bdmv_path = std::path::PathBuf::from(&mount).join("BDMV");
            if bdmv_path.exists() && bdmv_path.is_dir() {
                info!("Detected Blu-ray in {} (found BDMV directory)", device);
                return Ok(MediaType::BluRay);
            }
            
            // Check for VIDEO_TS directory which indicates DVD
            let video_ts_path = std::path::PathBuf::from(&mount).join("VIDEO_TS");
            if video_ts_path.exists() && video_ts_path.is_dir() {
                info!("Detected DVD in {} (found VIDEO_TS directory)", device);
                return Ok(MediaType::DVD);
            }
        }
        
        // If we can't check directory structure, try using disc size as a hint
        // Blu-rays are typically 25GB+ (or 50GB+), DVDs are typically 4.7GB or 8.5GB
        if let Some(size_line) = stdout.lines().find(|line| line.trim().starts_with("Disk Size:")) {
            if let Some(size_str) = size_line.split(':').nth(1) {
                // Extract size in bytes or GB
                let size_lower = size_str.trim().to_lowercase();
                if size_lower.contains("gb") {
                    if let Some(gb_str) = size_lower.split_whitespace().find(|s| s.parse::<f64>().is_ok()) {
                        if let Ok(gb) = gb_str.parse::<f64>() {
                            if gb >= 20.0 {
                                info!("Detected Blu-ray in {} (large disc size: {}GB)", device, gb);
                                return Ok(MediaType::BluRay);
                            }
                        }
                    }
                }
            }
        }
        
        // Fallback: default to DVD since it's more common
        warn!("Detected UDF disc in {}, assuming DVD (could not confirm type)", device);
        return Ok(MediaType::DVD);
    }
    
    warn!("Could not determine media type for {}", device);
    Ok(MediaType::None)
}

/// Detect media type on Linux using udev, mount points, and file system
#[cfg(target_os = "linux")]
async fn detect_media_type_linux(device: &str) -> Result<MediaType> {
    // Check if device exists
    if !std::path::Path::new(device).exists() {
        return Ok(MediaType::None);
    }
    
    // Try to get media type from udev
    if let Ok(output) = Command::new("udevadm")
        .arg("info")
        .arg("-q")
        .arg("property")
        .arg(device)
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Check ID_CDROM_MEDIA_* properties
        if stdout.contains("ID_CDROM_MEDIA_BD=") && stdout.contains("ID_CDROM_MEDIA_BD=1") {
            info!("Detected Blu-ray in {} (via udev)", device);
            return Ok(MediaType::BluRay);
        }
        
        if stdout.contains("ID_CDROM_MEDIA_DVD=") && stdout.contains("ID_CDROM_MEDIA_DVD=1") {
            info!("Detected DVD in {} (via udev)", device);
            return Ok(MediaType::DVD);
        }
        
        if stdout.contains("ID_CDROM_MEDIA_TRACK_COUNT_AUDIO=") {
            let track_count = stdout.lines()
                .find(|l| l.starts_with("ID_CDROM_MEDIA_TRACK_COUNT_AUDIO="))
                .and_then(|l| l.split('=').nth(1))
                .and_then(|s| s.parse::<u32>().ok());
            
            if track_count.is_some() && track_count.unwrap() > 0 {
                info!("Detected Audio CD in {} (via udev)", device);
                return Ok(MediaType::AudioCD);
            }
        }
    }
    
    // Check mount point and file system structure
    // First, find mount point using lsblk or mount
    let mount_point = if let Ok(output) = Command::new("lsblk")
        .arg("-n")
        .arg("-o")
        .arg("MOUNTPOINT")
        .arg(device)
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines()
            .next()
            .and_then(|s| {
                let trimmed = s.trim();
                if !trimmed.is_empty() && trimmed != "null" {
                    Some(trimmed.to_string())
                } else {
                    None
                }
            })
    } else {
        None
    };
    
    if let Some(mount) = mount_point {
        let mount_path = std::path::PathBuf::from(&mount);
        
        // Check for BDMV directory (Blu-ray)
        if mount_path.join("BDMV").exists() {
            info!("Detected Blu-ray in {} (found BDMV directory)", device);
            return Ok(MediaType::BluRay);
        }
        
        // Check for VIDEO_TS directory (DVD)
        if mount_path.join("VIDEO_TS").exists() {
            info!("Detected DVD in {} (found VIDEO_TS directory)", device);
            return Ok(MediaType::DVD);
        }
        
        // Check file system type
        if let Ok(output) = Command::new("findmnt")
            .arg("-n")
            .arg("-o")
            .arg("FSTYPE")
            .arg(&mount)
            .output()
        {
            let fstype = String::from_utf8_lossy(&output.stdout).trim().to_string();
            
            // UDF is used by both DVDs and Blu-rays, iso9660 is common for CDs
            if fstype == "iso9660" {
                // Could be Audio CD or data CD - check for CDDA
                info!("Detected Audio CD in {} (iso9660 filesystem)", device);
                return Ok(MediaType::AudioCD);
            }
        }
    }
    
    // Try to detect by trying to read disc info with cd-info or similar
    // For now, default to None if we can't determine
    warn!("Could not determine media type for {} on Linux", device);
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

/// Eject a disc from the specified drive (cross-platform)
pub async fn eject_disc(device: &str) -> Result<()> {
    info!("Ejecting disc from {}", device);
    
    #[cfg(target_os = "macos")]
    {
        eject_disc_macos(device).await
    }
    
    #[cfg(target_os = "linux")]
    {
        eject_disc_linux(device).await
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!("Eject not supported on this platform"))
    }
}

/// Eject disc on macOS using drutil
#[cfg(target_os = "macos")]
async fn eject_disc_macos(device: &str) -> Result<()> {
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

/// Eject disc on Linux using eject or udisksctl
#[cfg(target_os = "linux")]
async fn eject_disc_linux(device: &str) -> Result<()> {
    // Try udisksctl first (more modern, used by GNOME/KDE)
    let udisks_result = Command::new("udisksctl")
        .arg("power-off")
        .arg("-b")
        .arg(device)
        .output();
    
    if let Ok(output) = udisks_result {
        if output.status.success() {
            info!("Ejected disc using udisksctl");
            return Ok(());
        }
    }
    
    // Fallback to eject command (traditional, works everywhere)
    let eject_result = Command::new("eject")
        .arg(device)
        .output()
        .context("Failed to execute eject command")?;
    
    if !eject_result.status.success() {
        let err = String::from_utf8_lossy(&eject_result.stderr);
        warn!("Failed to eject disc: {}", err);
        return Err(anyhow::anyhow!("Eject failed: {}", err));
    }
    
    Ok(())
}

/// Unmount a disc from the specified drive (cross-platform)
pub async fn unmount_disc(device: &str) -> Result<()> {
    info!("Unmounting disc from {}", device);
    
    #[cfg(target_os = "macos")]
    {
        unmount_disc_macos(device).await
    }
    
    #[cfg(target_os = "linux")]
    {
        unmount_disc_linux(device).await
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!("Unmount not supported on this platform"))
    }
}

/// Unmount disc on macOS using diskutil
#[cfg(target_os = "macos")]
async fn unmount_disc_macos(device: &str) -> Result<()> {
    let output = Command::new("diskutil")
        .arg("unmountDisk")
        .arg("force")
        .arg(device)
        .output()
        .context("Failed to unmount disc")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        warn!("Failed to unmount disc: {}", err);
        return Err(anyhow::anyhow!("Unmount failed: {}", err));
    }

    Ok(())
}

/// Unmount disc on Linux using umount or udisksctl
#[cfg(target_os = "linux")]
async fn unmount_disc_linux(device: &str) -> Result<()> {
    // First, find the mount point using lsblk
    let mount_point = if let Ok(output) = Command::new("lsblk")
        .arg("-n")
        .arg("-o")
        .arg("MOUNTPOINT")
        .arg(device)
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines()
            .next()
            .and_then(|s| {
                let trimmed = s.trim();
                if !trimmed.is_empty() && trimmed != "null" {
                    Some(trimmed.to_string())
                } else {
                    None
                }
            })
    } else {
        None
    };
    
    if let Some(mount) = mount_point {
        // Try udisksctl first (more modern, works with permissions)
        let udisks_result = Command::new("udisksctl")
            .arg("unmount")
            .arg("-b")
            .arg(device)
            .output();
        
        if let Ok(output) = udisks_result {
            if output.status.success() {
                info!("Unmounted disc using udisksctl");
                return Ok(());
            }
        }
        
        // Fallback to umount command (requires root or proper permissions)
        let umount_result = Command::new("umount")
            .arg(&mount)
            .output();
        
        if let Ok(output) = umount_result {
            if output.status.success() {
                info!("Unmounted disc using umount");
                return Ok(());
            } else {
                let err = String::from_utf8_lossy(&output.stderr);
                warn!("umount failed: {}", err);
                // Don't fail if already unmounted
                if err.contains("not mounted") || err.contains("no mount point") {
                    return Ok(());
                }
            }
        }
    } else {
        // Device might not be mounted - that's okay
        debug!("Device {} is not mounted, skipping unmount", device);
        return Ok(());
    }
    
    // If we get here, unmount might have failed or device wasn't mounted
    // That's okay for our purposes - we'll proceed
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
