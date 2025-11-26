use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use crate::audio;
use crate::cli::Args;
use crate::drive::{self, DriveInfo};
use crate::metadata;
use crate::ripper;
use crate::tui::Tui;

pub async fn run(args: Args) -> Result<()> {
    // Get output folder (using default if not specified)
    let output_folder = args.get_output_folder();
    
    // Validate output directory
    if !output_folder.exists() {
        tokio::fs::create_dir_all(&output_folder).await?;
    }

    // Initialize sounds directory
    audio::initialize_sounds_dir().await?;

    // Create TUI
    let mut tui = Tui::new()?;
    let tui_state = Arc::clone(&tui.state);

    tui.add_log("🎵 Ripley started - monitoring for audio CDs...".to_string()).await;
    tui.add_log(format!("Output directory: {}", output_folder.display())).await;
    tui.add_log(format!("FLAC quality: {}", args.quality)).await;

    // Track active rip tasks
    let active_rips: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>> = 
        Arc::new(Mutex::new(HashMap::new()));

    // Spawn drive monitor task
    let args_clone = args.clone();
    let tui_state_clone = Arc::clone(&tui_state);
    let active_rips_clone = Arc::clone(&active_rips);

    tokio::spawn(async move {
        let result = drive::monitor_drives(move |drives| {
            let args = args_clone.clone();
            let tui_state = Arc::clone(&tui_state_clone);
            let active_rips = Arc::clone(&active_rips_clone);

            tokio::spawn(async move {
                handle_drive_changes(drives, args, tui_state, active_rips).await;
            });
        }).await;

        if let Err(e) = result {
            eprintln!("Drive monitoring error: {}", e);
        }
    });

    // Run TUI
    tui.run().await?;

    Ok(())
}

async fn handle_drive_changes(
    drives: Vec<DriveInfo>,
    args: Args,
    tui_state: Arc<Mutex<crate::tui::AppState>>,
    active_rips: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
) {
    for drive in drives {
        // Skip if no media
        if matches!(drive.media_type, drive::MediaType::None) {
            continue;
        }

        let mut rips = active_rips.lock().await;
        
        // Skip if already ripping this drive
        if rips.contains_key(&drive.device) {
            continue;
        }

        // Start ripping task
        info!("Starting rip task for {}", drive.device);
        
        let device = drive.device.clone();
        let device_for_task = device.clone();
        let args_clone = args.clone();
        let tui_state_clone = Arc::clone(&tui_state);
        let active_rips_clone = Arc::clone(&active_rips);

        let media_type = drive.media_type.clone();
        
        let handle = tokio::spawn(async move {
            let tui_state_for_error = Arc::clone(&tui_state_clone);
            if let Err(e) = rip_disc(&device_for_task, media_type, args_clone, tui_state_clone).await {
                tracing::error!("Rip task failed for {}: {}", device_for_task, e);
                let mut state = tui_state_for_error.lock().await;
                state.add_log(format!("❌ Error ripping {}: {}", device_for_task, e));
            }

            // Remove from active rips
            let mut rips = active_rips_clone.lock().await;
            rips.remove(&device_for_task);
        });

        rips.insert(device, handle);
    }
}

async fn rip_disc(
    device: &str,
    media_type: drive::MediaType,
    args: Args,
    tui_state: Arc<Mutex<crate::tui::AppState>>,
) -> Result<()> {
    // Helper to add logs without creating a full Tui
    async fn add_log(state: &Arc<Mutex<crate::tui::AppState>>, device: &str, msg: String) {
        let mut s = state.lock().await;
        s.add_drive_log(device, msg);
    }

    // Handle DVD/Blu-ray ripping (MakeMKV handles both)
    if matches!(media_type, drive::MediaType::DVD | drive::MediaType::BluRay) {
        return rip_dvd_disc(device, media_type.clone(), args, tui_state).await;
    }

    add_log(&tui_state, device, format!("📀 Detected audio CD in {}", device)).await;

    // Unmount disc before reading (cd-discid needs exclusive access)
    add_log(&tui_state, device, "💿 Preparing disc for reading...".to_string()).await;
    for attempt in 1..=3 {
        match tokio::process::Command::new("diskutil")
            .arg("unmountDisk")
            .arg("force")
            .arg(device)
            .output()
            .await {
            Ok(output) if output.status.success() => {
                tracing::info!("Unmounted {} for disc ID reading (attempt {})", device, attempt);
                break;
            }
            Ok(output) => {
                let err = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("Unmount attempt {}: {}", attempt, err);
                if attempt < 3 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
            Err(e) => {
                tracing::error!("Unmount command failed: {}", e);
            }
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    // Fetch metadata
    add_log(&tui_state, device, format!("🔍 Fetching metadata for {}...", device)).await;
    
    let disc_id = match metadata::get_disc_id(device).await {
        Ok(id) => {
            add_log(&tui_state, device, format!("📀 Disc ID: {}", id)).await;
            id
        }
        Err(e) => {
            add_log(&tui_state, device, format!("⚠️  Could not get disc ID: {}", e)).await;
            if args.skip_metadata {
                "unknown".to_string()
            } else {
                audio::play_notification("error").await?;
                return Err(e);
            }
        }
    };
    
    let metadata = if args.skip_metadata {
        // Create dummy metadata
        create_dummy_metadata()
    } else {
        match metadata::fetch_metadata(&disc_id, 3).await {
            Ok(meta) => {
                add_log(&tui_state, device, format!("✅ Found: {} - {} ({} tracks)", 
                    meta.artist, meta.album, meta.tracks.len())).await;
                meta
            }
            Err(e) => {
                add_log(&tui_state, device, format!("⚠️  Metadata lookup failed: {}", e)).await;
                add_log(&tui_state, device, "Using generic track names. You can rename files after ripping.".to_string()).await;
                audio::play_notification("error").await?;
                
                // Use dummy metadata - abcde will still rip the tracks
                create_dummy_metadata()
            }
        }
    };

    let album_info = format!("{} - {}", metadata.artist, metadata.album);
    
    // Update album info in the drive state
    {
        let mut s = tui_state.lock().await;
        if let Some(drive) = s.drives.iter_mut().find(|d| d.device == device) {
            drive.album_info = Some(album_info.clone());
        }
    }
    
    // Start ripping
    add_log(&tui_state, device, format!("🎵 Ripping {} from {}...", album_info, device)).await;

    let device_clone = device.to_string();
    let album_info_clone = album_info.clone();
    let tui_state_clone = Arc::clone(&tui_state);
    let tui_state_log_clone = Arc::clone(&tui_state);
    let device_log_clone = device.to_string();

    let output_folder = args.get_output_folder();
    let result = ripper::rip_cd(
        device,
        &metadata,
        &output_folder,
        args.quality,
        move |progress| {
            let device = device_clone.clone();
            let album_info = album_info_clone.clone();
            let tui_state = Arc::clone(&tui_state_clone);

            tokio::spawn(async move {
                let mut s = tui_state.lock().await;
                if let Some(drive) = s.drives.iter_mut().find(|d| d.device == device) {
                    drive.progress = Some(progress);
                    if drive.album_info.is_none() {
                        drive.album_info = Some(album_info);
                    }
                }
            });
        },
        move |log_line| {
            let device = device_log_clone.clone();
            let tui_state = Arc::clone(&tui_state_log_clone);
            tokio::spawn(async move {
                add_log(&tui_state, &device, log_line).await;
            });
        },
    ).await;

    match result {
        Ok(_) => {
            add_log(&tui_state, device, format!("✅ Completed: {}", album_info)).await;
            audio::play_notification("complete").await?;

            if args.eject_when_done {
                drive::eject_disc(device).await?;
                add_log(&tui_state, device, format!("⏏️  Ejected {}", device)).await;
            }
        }
        Err(e) => {
            add_log(&tui_state, device, format!("❌ Failed: {} - {}", album_info, e)).await;
            audio::play_notification("error").await?;
        }
    }

    // Remove drive from display
    {
        let mut state = tui_state.lock().await;
        state.drives.retain(|d| d.device != device);
    }

    Ok(())
}

async fn rip_dvd_disc(
    device: &str,
    media_type: drive::MediaType,
    args: Args,
    tui_state: Arc<Mutex<crate::tui::AppState>>,
) -> Result<()> {
    // Helper to add logs
    async fn add_log(state: &Arc<Mutex<crate::tui::AppState>>, device: &str, msg: String) {
        let mut s = state.lock().await;
        s.add_drive_log(device, msg);
    }

    let media_name = match media_type {
        drive::MediaType::BluRay => "Blu-ray",
        drive::MediaType::DVD => "DVD",
        _ => "disc",
    };
    
    add_log(&tui_state, device, format!("📀 Detected {} in {}", media_name, device)).await;

    // Try to get disc volume name and metadata
    add_log(&tui_state, device, format!("🔍 Fetching {} metadata...", media_name)).await;
    
    let volume_name = get_dvd_volume_name(device).await.ok();
    let dvd_metadata = if !args.skip_metadata {
        match crate::dvd_metadata::fetch_dvd_metadata("", volume_name.as_deref()).await {
            Ok(meta) => {
                add_log(&tui_state, device, format!("📺 Found: {}", meta.title)).await;
                if meta.media_type == crate::dvd_metadata::MediaType::TVShow && !meta.episodes.is_empty() {
                    add_log(&tui_state, device, format!("📝 {} episodes detected", meta.episodes.len())).await;
                }
                Some(meta)
            }
            Err(e) => {
                add_log(&tui_state, device, format!("⚠️  Could not fetch metadata: {}", e)).await;
                None
            }
        }
    } else {
        None
    };

    let default_label = match media_type {
        drive::MediaType::BluRay => "Blu-ray Video",
        drive::MediaType::DVD => "DVD Video",
        _ => "Video Disc",
    };
    
    let album_info = if let Some(ref meta) = dvd_metadata {
        if let Some(year) = &meta.year {
            format!("{} ({})", meta.title, year)
        } else {
            meta.title.clone()
        }
    } else {
        volume_name.unwrap_or_else(|| default_label.to_string())
    };

    // Update album info
    {
        let mut s = tui_state.lock().await;
        if let Some(drive) = s.drives.iter_mut().find(|d| d.device == device) {
            drive.album_info = Some(album_info.clone());
        }
    }

    let output_folder = args.get_output_folder();
    let media_output = match media_type {
        drive::MediaType::BluRay => output_folder.join("BluRays"),
        drive::MediaType::DVD => output_folder.join("DVDs"),
        _ => output_folder.join("Videos"),
    };
    
    // Create output folder with title or timestamp
    let folder_name = if let Some(ref meta) = dvd_metadata {
        crate::ripper::sanitize_filename(&meta.title)
    } else {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let prefix = match media_type {
            drive::MediaType::BluRay => "BluRay",
            drive::MediaType::DVD => "DVD",
            _ => "Video",
        };
        format!("{}_{}", prefix, timestamp)
    };
    let dvd_dir = media_output.join(folder_name);
    
    add_log(&tui_state, device, format!("Output: {}", dvd_dir.display())).await;

    let device_clone = device.to_string();
    let tui_state_clone = Arc::clone(&tui_state);
    let tui_state_log_clone = Arc::clone(&tui_state);
    let device_log_clone = device.to_string();

    let result = crate::dvd_ripper::rip_dvd(
        device,
        &dvd_dir,
        dvd_metadata.as_ref(),
        move |progress| {
            let device = device_clone.clone();
            let tui_state = Arc::clone(&tui_state_clone);

            tokio::spawn(async move {
                let mut s = tui_state.lock().await;
                if let Some(drive) = s.drives.iter_mut().find(|d| d.device == device) {
                    drive.progress = Some(progress);
                    if drive.album_info.is_none() {
                        drive.album_info = Some("DVD Video".to_string());
                    }
                }
            });
        },
        move |log_line| {
            let device = device_log_clone.clone();
            let tui_state = Arc::clone(&tui_state_log_clone);
            tokio::spawn(async move {
                add_log(&tui_state, &device, log_line).await;
            });
        },
    ).await;

    match result {
        Ok(_) => {
            add_log(&tui_state, device, format!("✅ {} rip complete", media_name)).await;
            audio::play_notification("complete").await?;

            if args.eject_when_done {
                drive::eject_disc(device).await?;
                add_log(&tui_state, device, format!("⏏️  Ejected {}", device)).await;
            }
        }
        Err(e) => {
            add_log(&tui_state, device, format!("❌ {} rip failed: {}", media_name, e)).await;
            audio::play_notification("error").await?;
        }
    }

    // Remove drive from display
    {
        let mut state = tui_state.lock().await;
        state.drives.retain(|d| d.device != device);
    }

    Ok(())
}

async fn get_dvd_volume_name(device: &str) -> Result<String> {
    let output = tokio::process::Command::new("drutil")
        .arg("status")
        .arg("-drive")
        .arg(device)
        .output()
        .await?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Extract volume label
    let volume_name = stdout.lines()
        .find(|line| line.contains("Name:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| anyhow::anyhow!("No volume name found"))?;
    
    Ok(volume_name)
}

fn create_dummy_metadata() -> metadata::DiscMetadata {
    // Try to get track count from the CD
    // For now, create a reasonable default - abcde will detect actual tracks
    let track_count = 10; // Default assumption
    
    let tracks: Vec<metadata::Track> = (1..=track_count)
        .map(|n| metadata::Track {
            number: n,
            title: format!("Track {:02}", n),
            artist: None,
            duration: None,
        })
        .collect();

    metadata::DiscMetadata {
        artist: "Unknown Artist".to_string(),
        album: format!("Unknown Album {}", chrono::Local::now().format("%Y-%m-%d")),
        year: Some(chrono::Local::now().format("%Y").to_string()),
        genre: None,
        tracks,
    }
}
