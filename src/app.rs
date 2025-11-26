use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

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
            
            // Send notification
            let disc_info = crate::notifications::DiscInfo {
                disc_type: crate::notifications::DiscType::CD,
                title: album_info.clone(),
                device: device.to_string(),
            };
            if let Err(e) = crate::notifications::send_completion_notification(disc_info, true).await {
                tracing::warn!("Failed to send notification: {}", e);
            }

            if args.eject_when_done {
                drive::eject_disc(device).await?;
                add_log(&tui_state, device, format!("⏏️  Ejected {}", device)).await;
            }
        }
        Err(e) => {
            add_log(&tui_state, device, format!("❌ Failed: {} - {}", album_info, e)).await;
            audio::play_notification("error").await?;
            
            // Send failure notification
            let disc_info = crate::notifications::DiscInfo {
                disc_type: crate::notifications::DiscType::CD,
                title: album_info.clone(),
                device: device.to_string(),
            };
            if let Err(e) = crate::notifications::send_completion_notification(disc_info, false).await {
                tracing::warn!("Failed to send notification: {}", e);
            }
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
    
    let volume_name = match get_dvd_volume_name(device).await {
        Ok(name) => {
            add_log(&tui_state, device, format!("💿 Volume name: {}", name)).await;
            Some(name)
        }
        Err(e) => {
            add_log(&tui_state, device, format!("⚠️  Could not get volume name: {}", e)).await;
            None
        }
    };
    
    // For DVDs/Blu-rays, prompt for title only (Filebot will handle episode matching)
    let title_to_search = if !args.skip_metadata {
        // Prompt for title (with default from --title flag or volume name)
        let default_title = args.title.clone().or(volume_name.clone());
        
        add_log(&tui_state, device, "📝 Please enter TV show title...".to_string()).await;
        {
            let mut state = tui_state.lock().await;
            state.input_mode = crate::tui::InputMode::AwaitingTitleInput {
                device: device.to_string(),
                default_title: default_title.clone(),
            };
            state.current_input = default_title.clone().unwrap_or_default();
        }
        
        // Wait for title input
        let title = loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let state = tui_state.lock().await;
            
            if matches!(state.input_mode, crate::tui::InputMode::Normal) {
                // Input was submitted or cancelled
                break state.current_input.clone();
            }
        };
        
        if title.is_empty() {
            add_log(&tui_state, device, "❌ No title provided, skipping metadata".to_string()).await;
            None
        } else {
            add_log(&tui_state, device, format!("📺 Using title: '{}'", title)).await;
            Some(title)
        }
    } else {
        add_log(&tui_state, device, "⏭️  Skipping metadata (--skip-metadata)".to_string()).await;
        None
    };
    
    let dvd_metadata = if let Some(ref title) = title_to_search {
        add_log(&tui_state, device, format!("🔍 Searching TMDB for '{}'...", title)).await;
        
        match crate::dvd_metadata::fetch_dvd_metadata("", Some(title.as_str())).await {
            Ok(meta) => {
                add_log(&tui_state, device, format!("📺 Found: {}", meta.title)).await;
                if meta.media_type == crate::dvd_metadata::MediaType::TVShow && !meta.episodes.is_empty() {
                    add_log(&tui_state, device, format!("📝 {} episodes available for matching", meta.episodes.len())).await;
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

    // Music CDs use the configured output folder, videos use ~/Desktop/Rips
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let base_rips = std::path::PathBuf::from(home).join("Desktop").join("Rips");
    
    let media_output = match media_type {
        drive::MediaType::BluRay => base_rips.join("BluRays"),
        drive::MediaType::DVD => base_rips.join("DVDs"),
        _ => base_rips.join("Videos"),
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
            
            // Run OCR + Filebot by default (unless --skip-filebot) if we have metadata
            if !args.skip_filebot && dvd_metadata.is_some() {
                let metadata = dvd_metadata.as_ref().unwrap();
                if metadata.media_type == crate::dvd_metadata::MediaType::TVShow {
                    // Step 1: Run OCR on all videos to extract episode titles
                    add_log(&tui_state, device, "🔍 Running OCR to extract episode titles (this may take a while)...".to_string()).await;
                    
                    let mut ocr_success_count = 0;
                    let show_title_lower = metadata.title.to_lowercase();
                    let mut read_dir = tokio::fs::read_dir(&dvd_dir).await?;
                    while let Some(entry) = read_dir.next_entry().await? {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("mkv") {
                            if let Ok(Some(title)) = crate::ocr::extract_episode_title(&path).await {
                                // Skip if OCR result looks like the show title (not episode title)
                                let title_lower = title.to_lowercase();
                                if title_lower.contains(&show_title_lower) || 
                                   title_lower.contains("home") && title_lower.contains("imaginary") {
                                    add_log(&tui_state, device, format!("  ⏭️  Skipped show title: {}", title)).await;
                                    continue;
                                }
                                
                                // Rename file to include OCR'd title
                                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                                let new_name = format!("{}.{}.mkv", filename, title.replace(' ', "."));
                                let new_path = dvd_dir.join(&new_name);
                                
                                if let Err(e) = tokio::fs::rename(&path, &new_path).await {
                                    add_log(&tui_state, device, format!("⚠️  Failed to rename with OCR title: {}", e)).await;
                                } else {
                                    add_log(&tui_state, device, format!("  ✓ {}", title)).await;
                                    ocr_success_count += 1;
                                }
                            }
                        }
                    }
                    
                    if ocr_success_count > 0 {
                        add_log(&tui_state, device, format!("✅ OCR extracted {} episode titles", ocr_success_count)).await;
                    } else {
                        add_log(&tui_state, device, "⚠️  OCR didn't find episode title cards (will use duration matching)".to_string()).await;
                    }
                    
                    // Step 2: Run Filebot with OCR-enhanced filenames
                    add_log(&tui_state, device, "🤖 Running Filebot to match with database...".to_string()).await;
                    
                    let dvd_dir_clone = dvd_dir.clone();
                    let show_title = metadata.title.clone();
                    let tui_state_filebot = Arc::clone(&tui_state);
                    let device_filebot = device.to_string();
                    
                    match crate::filebot::rename_with_filebot(
                        &dvd_dir_clone,
                        &show_title,
                        move |log_msg| {
                            let device = device_filebot.clone();
                            let tui_state = Arc::clone(&tui_state_filebot);
                            tokio::spawn(async move {
                                add_log(&tui_state, &device, log_msg).await;
                            });
                        }
                    ).await {
                        Ok(_) => {
                            add_log(&tui_state, device, "✅ Filebot renaming complete".to_string()).await;
                        }
                        Err(e) => {
                            add_log(&tui_state, device, format!("⚠️  Filebot failed: {}", e)).await;
                        }
                    }
                }
            }
            
            // Send notification
            let disc_type = match media_type {
                drive::MediaType::BluRay => crate::notifications::DiscType::BluRay,
                drive::MediaType::DVD => crate::notifications::DiscType::DVD,
                _ => crate::notifications::DiscType::DVD,
            };
            let disc_info = crate::notifications::DiscInfo {
                disc_type,
                title: album_info.clone(),
                device: device.to_string(),
            };
            if let Err(e) = crate::notifications::send_completion_notification(disc_info, true).await {
                tracing::warn!("Failed to send notification: {}", e);
            }
            
            // Start rsync in background for DVD/Blu-ray
            add_log(&tui_state, device, "📤 Starting rsync to /Volumes/video/RawRips...".to_string()).await;
            let dvd_dir_clone = dvd_dir.clone();
            let tui_state_rsync = Arc::clone(&tui_state);
            tokio::spawn(async move {
                if let Err(e) = crate::rsync::rsync_to_rawrips(&dvd_dir_clone, tui_state_rsync).await {
                    tracing::warn!("Rsync failed: {}", e);
                }
            });

            if args.eject_when_done {
                drive::eject_disc(device).await?;
                add_log(&tui_state, device, format!("⏏️  Ejected {}", device)).await;
            }
        }
        Err(e) => {
            add_log(&tui_state, device, format!("❌ {} rip failed: {}", media_name, e)).await;
            audio::play_notification("error").await?;
            
            // Send failure notification
            let disc_type = match media_type {
                drive::MediaType::BluRay => crate::notifications::DiscType::BluRay,
                drive::MediaType::DVD => crate::notifications::DiscType::DVD,
                _ => crate::notifications::DiscType::DVD,
            };
            let disc_info = crate::notifications::DiscInfo {
                disc_type,
                title: album_info.clone(),
                device: device.to_string(),
            };
            if let Err(e) = crate::notifications::send_completion_notification(disc_info, false).await {
                tracing::warn!("Failed to send notification: {}", e);
            }
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
    debug!("Getting volume name for device: {}", device);
    
    // Use diskutil info to get the Volume Name field (more reliable than drutil)
    let output = tokio::process::Command::new("diskutil")
        .arg("info")
        .arg(device)
        .output()
        .await?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    debug!("diskutil info output for volume name extraction:\n{}", stdout);
    
    // Extract volume label from "Volume Name:" field
    let volume_name = stdout.lines()
        .find(|line| line.trim().starts_with("Volume Name:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "Not applicable (no file system)")
        .ok_or_else(|| {
            warn!("No 'Volume Name:' field found in diskutil output");
            anyhow::anyhow!("No volume name found")
        })?;
    
    debug!("Extracted volume name: {}", volume_name);
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
