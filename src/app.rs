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
    // Validate output directory
    if !args.output_folder.exists() {
        tokio::fs::create_dir_all(&args.output_folder).await?;
    }

    // Initialize sounds directory
    audio::initialize_sounds_dir().await?;

    // Create TUI
    let mut tui = Tui::new()?;
    let tui_state = Arc::clone(&tui.state);

    tui.add_log("🎵 Ripley started - monitoring for audio CDs...".to_string()).await;
    tui.add_log(format!("Output directory: {}", args.output_folder.display())).await;
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
        if !drive.has_audio_cd {
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

        let handle = tokio::spawn(async move {
            let tui_state_for_error = Arc::clone(&tui_state_clone);
            if let Err(e) = rip_disc(&device_for_task, args_clone, tui_state_clone).await {
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
    args: Args,
    tui_state: Arc<Mutex<crate::tui::AppState>>,
) -> Result<()> {
    // Helper to add logs without creating a full Tui
    async fn add_log(state: &Arc<Mutex<crate::tui::AppState>>, msg: String) {
        let mut s = state.lock().await;
        s.add_log(msg);
    }

    add_log(&tui_state, format!("📀 Detected audio CD in {}", device)).await;

    // Fetch metadata
    add_log(&tui_state, format!("🔍 Fetching metadata for {}...", device)).await;
    
    let disc_id = metadata::get_disc_id(device).await?;
    let metadata = if args.skip_metadata {
        // Create dummy metadata
        create_dummy_metadata()
    } else {
        match metadata::fetch_metadata(&disc_id, 3).await {
            Ok(meta) => {
                add_log(&tui_state, format!("✅ Found: {} - {}", meta.artist, meta.album)).await;
                meta
            }
            Err(e) => {
                add_log(&tui_state, format!("⚠️  Metadata lookup failed: {}", e)).await;
                audio::play_notification("error").await?;
                
                // TODO: Prompt user for manual input
                // For now, use dummy metadata
                create_dummy_metadata()
            }
        }
    };

    let album_info = format!("{} - {}", metadata.artist, metadata.album);
    
    // Start ripping
    add_log(&tui_state, format!("🎵 Ripping {} from {}...", album_info, device)).await;

    let device_clone = device.to_string();
    let album_info_clone = album_info.clone();
    let tui_state_clone = Arc::clone(&tui_state);

    let result = ripper::rip_cd(
        device,
        &metadata,
        &args.output_folder,
        args.quality,
        move |progress| {
            let device = device_clone.clone();
            let album_info = album_info_clone.clone();
            let tui_state = Arc::clone(&tui_state_clone);

            tokio::spawn(async move {
                async fn update_drive(
                    state: Arc<Mutex<crate::tui::AppState>>,
                    device: String,
                    progress: ripper::RipProgress,
                    album_info: Option<String>,
                ) {
                    let mut s = state.lock().await;
                    if let Some(drive) = s.drives.iter_mut().find(|d| d.device == device) {
                        drive.progress = Some(progress);
                        if album_info.is_some() {
                            drive.album_info = album_info;
                        }
                    } else {
                        s.drives.push(crate::tui::DriveState {
                            device,
                            progress: Some(progress),
                            album_info,
                        });
                    }
                }
                
                update_drive(tui_state, device, progress, Some(album_info)).await;
            });
        },
    ).await;

    match result {
        Ok(_) => {
            add_log(&tui_state, format!("✅ Completed: {}", album_info)).await;
            audio::play_notification("complete").await?;

            if args.eject_when_done {
                drive::eject_disc(device).await?;
                add_log(&tui_state, format!("⏏️  Ejected {}", device)).await;
            }
        }
        Err(e) => {
            add_log(&tui_state, format!("❌ Failed: {} - {}", album_info, e)).await;
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

fn create_dummy_metadata() -> metadata::DiscMetadata {
    metadata::DiscMetadata {
        artist: "Unknown Artist".to_string(),
        album: "Unknown Album".to_string(),
        year: None,
        genre: None,
        tracks: vec![
            metadata::Track {
                number: 1,
                title: "Track 1".to_string(),
                artist: None,
                duration: None,
            },
        ],
    }
}
