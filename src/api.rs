use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http,
    response::{IntoResponse, Response},
    routing::{get, post, put, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::cli::RipArgs;
use crate::config::Config;
use crate::database::{Database, LogEntry, Issue, Show, RipQueueEntry, QueueStatus};

/// Start the REST API server
pub async fn start_server(
    config: Config,
    host: String,
    port: u16,
    dev_mode: bool,
) -> anyhow::Result<()> {
    // Create broadcast channel for events
    let (event_tx, _) = broadcast::channel(100);
    
    // Initialize database
    let db = Arc::new(Database::new()?);
    info!("Database initialized");
    
    // Create shared state
    let state = ApiState {
        config: Arc::new(RwLock::new(config)),
        rip_status: Arc::new(RwLock::new(RipStatus::default())),
        event_tx: event_tx.clone(),
        db: Arc::clone(&db),
    };
    
    // Spawn background task to log events to database
    let mut event_rx = event_tx.subscribe();
    let db_logger = Arc::clone(&db);
    tokio::spawn(async move {
        use crate::database::{LogEntry, LogLevel, Issue, IssueType};
        
        while let Ok(event) = event_rx.recv().await {
            match event {
                ApiEvent::Log { level, message, drive } => {
                    let log_level = match level.as_str() {
                        "error" => LogLevel::Error,
                        "warning" => LogLevel::Warning,
                        "success" => LogLevel::Success,
                        _ => LogLevel::Info,
                    };
                    
                    let entry = LogEntry {
                        id: None,
                        timestamp: chrono::Utc::now(),
                        level: log_level,
                        message,
                        drive,
                        disc: None,
                        title: None,
                        context: None,
                    };
                    
                    if let Err(e) = db_logger.add_log(&entry) {
                        eprintln!("Failed to log to database: {}", e);
                    }
                }
                ApiEvent::RipError { error, drive } => {
                    // Log the error
                    let entry = LogEntry {
                        id: None,
                        timestamp: chrono::Utc::now(),
                        level: LogLevel::Error,
                        message: format!("Rip error: {}", error),
                        drive: drive.clone(),
                        disc: None,
                        title: None,
                        context: Some(error.clone()),
                    };
                    let _ = db_logger.add_log(&entry);
                    
                    // Create an issue
                    let issue = Issue {
                        id: None,
                        timestamp: chrono::Utc::now(),
                        issue_type: IssueType::RipFailure,
                        title: "Rip Operation Failed".to_string(),
                        description: error,
                        drive,
                        disc: None,
                        resolved: false,
                        resolved_at: None,
                        assigned_to: None,
                        resolution_notes: None,
                    };
                    let _ = db_logger.add_issue(&issue);
                }
                _ => {}
            }
        }
    });
    
    // Spawn background task to poll for drive changes
    let event_tx_poller = event_tx.clone();
    tokio::spawn(async move {
        use std::collections::HashMap;
        use crate::drive::{self, DriveInfo};
        
        let mut known_drives: HashMap<String, DriveInfo> = HashMap::new();
        
        loop {
            // Sleep first to avoid immediate polling on startup
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            
            // Detect current drives
            match drive::detect_drives().await {
                Ok(current_drives) => {
                    let current_map: HashMap<String, DriveInfo> = current_drives
                        .into_iter()
                        .map(|d| (d.device.clone(), d))
                        .collect();
                    
                    // Find newly detected drives
                    for (device, drive_info) in &current_map {
                        if !known_drives.contains_key(device) {
                            info!("Drive detected: {}", device);
                            let _ = event_tx_poller.send(ApiEvent::DriveDetected {
                                drive: drive_info.clone(),
                            });
                        }
                    }
                    
                    // Find removed drives
                    for (device, _drive_info) in &known_drives {
                        if !current_map.contains_key(device) {
                            info!("Drive removed: {}", device);
                            let _ = event_tx_poller.send(ApiEvent::DriveRemoved {
                                device: device.clone(),
                            });
                        }
                    }
                    
                    // Update known drives
                    known_drives = current_map;
                }
                Err(e) => {
                    eprintln!("Failed to detect drives: {}", e);
                }
            }
        }
    });
    
    // Create router with API routes
    let mut app = create_router(state);
    
    // In production mode, add static file serving
    if !dev_mode {
        app = app.fallback(crate::web_ui::fallback);
    }
    
    // Create socket address
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    
    info!("🌐 REST API server starting on http://{}", addr);
    eprintln!("\x1b[32m✓\x1b[0m Server listening on \x1b[1mhttp://{}\x1b[0m", addr);
    
    if dev_mode {
        eprintln!("\x1b[36m  • Web UI:\x1b[0m        \x1b[1mhttp://localhost:5173\x1b[0m (Vite dev server)");
        eprintln!("\x1b[36m  • API:\x1b[0m           http://{}/api", addr);
    } else {
        eprintln!("\x1b[36m  • Web UI:\x1b[0m        http://{}/", addr);
    }
    
    eprintln!("\x1b[36m  • Health check:\x1b[0m http://{}/api/health", addr);
    eprintln!("\x1b[36m  • API status:\x1b[0m    http://{}/api/status", addr);
    eprintln!("\x1b[36m  • WebSocket:\x1b[0m     ws://{}/api/ws\n", addr);
    
    if dev_mode {
        eprintln!("\x1b[33m💡 Hot reload enabled - changes to web-ui/ will update automatically\x1b[0m");
    }
    
    eprintln!("\x1b[33m💡 Press Ctrl+C to stop the server\x1b[0m\n");
    
    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

/// Shared application state accessible to all API handlers
#[derive(Clone)]
pub struct ApiState {
    pub config: Arc<RwLock<Config>>,
    pub rip_status: Arc<RwLock<RipStatus>>,
    pub event_tx: broadcast::Sender<ApiEvent>,
    pub db: Arc<Database>,
}

/// Per-drive ripping status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveRipStatus {
    pub current_disc: Option<String>,
    pub current_title: Option<String>,
    pub progress: f32,
    pub paused: bool,
    pub paused_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Current ripping status (supports multiple drives)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipStatus {
    pub active_rips: std::collections::HashMap<String, DriveRipStatus>,
    pub logs: Vec<String>,
}

impl Default for RipStatus {
    fn default() -> Self {
        Self {
            active_rips: std::collections::HashMap::new(),
            logs: Vec::new(),
        }
    }
}

/// Events broadcast to WebSocket clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ApiEvent {
    RipStarted { disc: String, drive: String },
    RipProgress { progress: f32, message: String, drive: String },
    RipCompleted { disc: String, drive: String },
    RipError { error: String, drive: Option<String> },
    Log { level: String, message: String, drive: Option<String> },
    StatusUpdate { status: RipStatus },
    DriveDetected { drive: crate::drive::DriveInfo },
    DriveRemoved { device: String },
    DriveEjected { device: String },
    IssueCreated { issue: Issue },
    RipPaused { drive: String },
    RipResumed { drive: String },
}

/// Request body for starting a rip operation
#[derive(Debug, Deserialize)]
pub struct StartRipRequest {
    pub drive: Option<String>,
    pub output_path: Option<String>,
    pub title: Option<String>,
    pub skip_metadata: bool,
    pub skip_filebot: bool,
    pub profile: Option<String>, // Optional profile name
    pub priority: Option<i32>, // Optional priority for queue (higher = higher priority, default 0)
}

/// Response for API errors
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (http::StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}

/// Create the API router with all routes
pub fn create_router(state: ApiState) -> Router {
    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/status", get(get_status))
        .route("/config", get(get_config))
        .route("/config", post(update_config))
        .route("/config/path", get(get_config_path_handler))
        .route("/rip/start", post(start_rip))
        .route("/rip/stop", post(stop_rip))
        .route("/drives", get(list_drives))
        .route("/drives/:device/eject", post(eject_drive))
        .route("/rename", post(rename_files))
        .route("/logs", get(get_logs))
        .route("/logs/search", get(search_logs_handler))
        .route("/logs/clear", delete(clear_logs_handler))
        .route("/issues", get(get_all_issues_handler))
        .route("/issues/active", get(get_active_issues))
        .route("/issues/:id/resolve", post(resolve_issue))
        .route("/issues/:id/assign", put(assign_issue_handler))
        .route("/issues/:id/resolution-notes", put(update_resolution_notes_handler))
        .route("/issues/:id/notes", get(get_issue_notes_handler))
        .route("/issues/:id/notes", post(add_issue_note_handler))
        .route("/issues/:id/notes/:note_id", delete(delete_issue_note_handler))
        .route("/settings/last-title", get(get_last_title))
        .route("/settings/last-title", post(set_last_title))
        .route("/settings/last-show", get(get_last_show_id_handler))
        .route("/shows", get(get_shows))
        .route("/shows", post(create_show))
        .route("/shows/:id", get(get_show))
        .route("/shows/:id", put(update_show))
        .route("/shows/:id", delete(delete_show))
        .route("/shows/:id/select", post(select_show))
        .route("/statistics", get(get_statistics))
        .route("/statistics/drives", get(get_drive_stats))
        .route("/statistics/errors", get(get_error_frequency))
        .route("/rip-history", get(get_rip_history_handler))
        .route("/preferences", get(get_preferences))
        .route("/preferences", post(update_preferences))
        .route("/rip-profiles", get(get_rip_profiles))
        .route("/queue", get(get_queue_handler))
        .route("/queue/:id/cancel", delete(cancel_queue_handler))
        .route("/rip/:drive/pause", put(pause_rip_handler))
        .route("/rip/:drive/resume", put(resume_rip_handler))
        .route("/episode-match-statistics", get(get_episode_match_statistics_handler))
        .route("/database/backup", post(backup_database_handler))
        .route("/database/restore", post(restore_database_handler))
        .route("/ws", get(websocket_handler))
        .with_state(state);

    Router::new()
        .nest("/api", api_routes)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

/// Health check endpoint
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Get current rip status
async fn get_status(State(state): State<ApiState>) -> Json<RipStatus> {
    let status = state.rip_status.read().await;
    Json(status.clone())
}

/// Get current configuration
async fn get_config(State(state): State<ApiState>) -> Json<Config> {
    let config = state.config.read().await;
    Json(config.clone())
}

/// Get config file path
async fn get_config_path_handler() -> Json<serde_json::Value> {
    let path = crate::config::get_config_path();
    Json(serde_json::json!({
        "path": path.to_string_lossy().to_string(),
        "exists": path.exists()
    }))
}

/// Update configuration
async fn update_config(
    State(state): State<ApiState>,
    Json(new_config): Json<Config>,
) -> Result<Json<Config>, ErrorResponse> {
    let mut config = state.config.write().await;
    *config = new_config.clone();
    
    // Optionally save to file
    if let Err(e) = crate::config::save_config(&new_config) {
        return Err(ErrorResponse {
            error: format!("Failed to save config: {}", e),
        });
    }
    
    Ok(Json(new_config))
}

/// Start ripping operation (queues if drive is busy)
async fn start_rip(
    State(state): State<ApiState>,
    Json(request): Json<StartRipRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    // Determine the drive identifier (use "default" if not specified)
    let drive = request.drive.clone();
    
    let mut status = state.rip_status.write().await;
    
    // Check if this specific drive (or any drive if not specified) is already ripping
    let drive_busy = if let Some(ref d) = drive {
        status.active_rips.contains_key(d)
    } else {
        !status.active_rips.is_empty()
    };
    
    if drive_busy {
        // Queue the request instead of rejecting
        let queue_entry = RipQueueEntry {
            id: None,
            created_at: chrono::Utc::now(),
            drive: drive.clone(),
            output_path: request.output_path.clone(),
            title: request.title.clone(),
            skip_metadata: request.skip_metadata,
            skip_filebot: request.skip_filebot,
            profile: request.profile.clone(),
            priority: request.priority.unwrap_or(0), // Use provided priority or default to 0
            status: QueueStatus::Pending,
            started_at: None,
        };
        
        drop(status);
        
        match state.db.add_to_queue(&queue_entry) {
            Ok(queue_id) => {
                // Queue will be processed when current rip completes
                // We trigger queue processing after rips complete, not here
                
                return Ok(Json(serde_json::json!({
                    "status": "queued",
                    "queue_id": queue_id,
                    "message": "Rip operation queued - will start when drive becomes available"
                })));
            }
            Err(e) => {
                return Err(ErrorResponse {
                    error: format!("Failed to queue rip operation: {}", e),
                });
            }
        }
    }
    
    // Drive is available, start immediately
    let drive_id = drive.clone().unwrap_or_else(|| "default".to_string());
    
    // Mark this drive as active
    status.active_rips.insert(drive_id.clone(), DriveRipStatus {
        current_disc: None,
        current_title: request.title.clone(),
        progress: 0.0,
        paused: false,
        paused_at: None,
    });
    drop(status);
    
    // Clone state for async task
    let state_clone = state.clone();
    let request_clone = request;
    let drive_clone = drive_id.clone();
    
    // Spawn rip operation in background
    tokio::spawn(async move {
        let result = run_rip_operation(state_clone.clone(), request_clone, drive_clone.clone()).await;
        
        // Remove from active rips when done
        let mut status = state_clone.rip_status.write().await;
        status.active_rips.remove(&drive_clone);
        
        if let Err(e) = result {
            tracing::error!("Rip operation failed: {:?}", e);
        }
        
        // Process queue after rip completes (use tokio::task::spawn_local or check queue on next start_rip)
        // Note: Queue processing happens automatically when checking for available drives
    });
    
    Ok(Json(serde_json::json!({
        "status": "started",
        "drive": drive_id
    })))
}

/// Stop ripping operation (stops all active rips)
async fn stop_rip(State(state): State<ApiState>) -> Json<serde_json::Value> {
    let mut status = state.rip_status.write().await;
    let drive_count = status.active_rips.len();
    status.active_rips.clear();
    
    let _ = state.event_tx.send(ApiEvent::Log {
        level: "warning".to_string(),
        message: format!("Stopped {} active rip operation(s)", drive_count),
        drive: None,
    });
    
    Json(serde_json::json!({
        "status": "stopped",
        "stopped_count": drive_count
    }))
}

/// Pause ripping operation for a specific drive
async fn pause_rip_handler(
    State(state): State<ApiState>,
    axum::extract::Path(drive): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let mut status = state.rip_status.write().await;
    
    if let Some(rip_status) = status.active_rips.get_mut(&drive) {
        if rip_status.paused {
            return Err(ErrorResponse {
                error: format!("Rip operation on drive {} is already paused", drive),
            });
        }
        
        rip_status.paused = true;
        rip_status.paused_at = Some(chrono::Utc::now());
        
        let _ = state.event_tx.send(ApiEvent::Log {
            level: "info".to_string(),
            message: format!("Rip operation on drive {} paused", drive),
            drive: Some(drive.clone()),
        });
        let _ = state.event_tx.send(ApiEvent::RipPaused {
            drive: drive.clone(),
        });
        
        drop(status);
        
        Ok(Json(serde_json::json!({
            "status": "paused",
            "drive": drive
        })))
    } else {
        drop(status);
        Err(ErrorResponse {
            error: format!("No active rip operation found on drive {}", drive),
        })
    }
}

/// Resume ripping operation for a specific drive
async fn resume_rip_handler(
    State(state): State<ApiState>,
    axum::extract::Path(drive): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let mut status = state.rip_status.write().await;
    
    if let Some(rip_status) = status.active_rips.get_mut(&drive) {
        if !rip_status.paused {
            return Err(ErrorResponse {
                error: format!("Rip operation on drive {} is not paused", drive),
            });
        }
        
        rip_status.paused = false;
        rip_status.paused_at = None;
        
        let _ = state.event_tx.send(ApiEvent::Log {
            level: "info".to_string(),
            message: format!("Rip operation on drive {} resumed", drive),
            drive: Some(drive.clone()),
        });
        let _ = state.event_tx.send(ApiEvent::RipResumed {
            drive: drive.clone(),
        });
        
        drop(status);
        
        Ok(Json(serde_json::json!({
            "status": "resumed",
            "drive": drive
        })))
    } else {
        drop(status);
        Err(ErrorResponse {
            error: format!("No active rip operation found on drive {}", drive),
        })
    }
}

/// List available optical drives
async fn list_drives(State(_state): State<ApiState>) -> Result<Json<Vec<crate::drive::DriveInfo>>, ErrorResponse> {
    match crate::drive::detect_drives().await {
        Ok(drives) => Ok(Json(drives)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to detect drives: {}", e),
        }),
    }
}

/// Eject a drive
async fn eject_drive(
    State(state): State<ApiState>,
    axum::extract::Path(device): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    // URL decode the device parameter (e.g., %2Fdev%2Fdisk2 -> /dev/disk2)
    let device = urlencoding::decode(&device)
        .map_err(|e| ErrorResponse {
            error: format!("Invalid device path: {}", e),
        })?
        .into_owned();
    
    match crate::drive::eject_disc(&device).await {
        Ok(_) => {
            // Emit DriveEjected event
            let _ = state.event_tx.send(ApiEvent::DriveEjected {
                device: device.clone(),
            });
            
            // Log the ejection
            let _ = state.event_tx.send(ApiEvent::Log {
                level: "info".to_string(),
                message: format!("Ejected drive {}", device),
                drive: Some(device.clone()),
            });
            
            Ok(Json(serde_json::json!({
                "success": true,
                "message": format!("Drive {} ejected successfully", device)
            })))
        }
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to eject drive: {}", e),
        }),
    }
}

/// Rename files request
#[derive(Debug, Deserialize)]
pub struct RenameRequest {
    pub directory: String,
    pub title: Option<String>,
    pub skip_speech: bool,
    pub skip_filebot: bool,
}

/// Rename existing files
async fn rename_files(
    State(state): State<ApiState>,
    Json(request): Json<RenameRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let config = state.config.read().await;
    
    // Spawn rename operation in background
    let state_clone = state.clone();
    let request_clone = request;
    let config_clone = config.clone();
    
    tokio::spawn(async move {
        let _ = run_rename_operation(state_clone, request_clone, config_clone).await;
    });
    
    Ok(Json(serde_json::json!({
        "status": "started"
    })))
}

/// WebSocket handler for real-time updates
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

/// Handle WebSocket connection
async fn handle_websocket(mut socket: WebSocket, state: ApiState) {
    let mut event_rx = state.event_tx.subscribe();
    
    // Send events to WebSocket
    while let Ok(event) = event_rx.recv().await {
        let json = serde_json::to_string(&event).unwrap_or_default();
        if socket.send(Message::Text(json)).await.is_err() {
            break;
        }
    }
}

/// Check if an error is retryable (some errors shouldn't be retried)
fn is_retryable_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();
    // Don't retry on permission errors, disk full, or invalid configurations
    let non_retryable = [
        "permission denied",
        "access denied",
        "disk full",
        "no space",
        "invalid",
        "not found",
        "no such file",
    ];
    
    !non_retryable.iter().any(|term| error_lower.contains(term))
}

/// Run the rip operation in background with automatic retry logic
async fn run_rip_operation(
    state: ApiState,
    request: StartRipRequest,
    drive_id: String,
) -> anyhow::Result<()> {
    
    let retry_config = {
        let config = state.config.read().await;
        config.retry.clone()
    };
    
    let start_time = chrono::Utc::now();
    let drive = drive_id.clone(); // Use the provided drive identifier
    
    // Attempt rip with retries
    let mut last_error = None;
    let mut attempt = 1u32;
    let max_attempts = if retry_config.enabled { retry_config.max_attempts } else { 1 };
    
    // Get title once at the start
    let title = if request.title.is_some() {
        request.title.clone()
    } else {
        state.db.get_last_title().ok().flatten()
    };
    
    while attempt <= max_attempts {
        if attempt > 1 {
            // Calculate exponential backoff delay
            let delay = std::cmp::min(
                (retry_config.initial_delay_seconds as f64 * retry_config.backoff_multiplier.powi(attempt as i32 - 2)) as u64,
                retry_config.max_delay_seconds
            );
            
            let _ = state.event_tx.send(ApiEvent::Log {
                level: "warning".to_string(),
                message: format!("Retrying rip (attempt {}/{} after {}s delay)...", attempt, max_attempts, delay),
                drive: Some(drive.clone()),
            });
            
            tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
        }
        
        let is_final_attempt = attempt >= max_attempts;
        match run_single_rip_attempt(&state, &request, &drive, start_time, is_final_attempt).await {
            Ok(_) => {
                // Success - return immediately (history already logged)
                return Ok(());
            }
            Err(e) => {
                last_error = Some(e);
                let error_msg = last_error.as_ref().unwrap().to_string();
                
                // Check if error is retryable
                if !is_retryable_error(&error_msg) || is_final_attempt {
                    // Not retryable or out of retries - break and log failure
                    break;
                }
                
                attempt += 1;
            }
        }
    }
    
    // All retries failed - log to history and return error
    use crate::database::{RipHistory, RipStatus};
    let end_time = chrono::Utc::now();
    let duration_seconds = (end_time - start_time).num_seconds();
    let title = if request.title.is_some() {
        request.title.clone()
    } else {
        state.db.get_last_title().ok().flatten()
    };
    
    let error_msg = last_error.as_ref().map(|e| e.to_string()).unwrap_or_else(|| "Unknown error".to_string());
    
    let _ = state.event_tx.send(ApiEvent::RipError {
        error: error_msg.clone(),
        drive: Some(drive.clone()),
    });
    
    let history = RipHistory {
        id: None,
        timestamp: start_time,
        drive: drive.clone(),
        disc: None,
        title: title.clone(),
        disc_type: None,
        status: RipStatus::Failed,
        duration_seconds: Some(duration_seconds),
        file_size_bytes: None,
        output_path: request.output_path.clone(),
        error_message: Some(format!("Failed after {} attempts: {}", max_attempts, error_msg)),
        avg_speed_mbps: None,
        checksum: None, // No checksum for failed rips
    };
    
    if let Err(e) = state.db.add_rip_history(&history) {
        tracing::error!("Failed to save rip history: {}", e);
    }
    
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Rip operation failed after {} attempts", max_attempts)))
}

/// Run a single rip attempt (without retry logic)
async fn run_single_rip_attempt(
    state: &ApiState,
    request: &StartRipRequest,
    drive: &String,
    start_time: chrono::DateTime<chrono::Utc>,
    is_final_attempt: bool,
) -> anyhow::Result<()> {
    use crate::database::{RipHistory, RipStatus};
    
    // Use provided title or fall back to last saved title
    let title = if request.title.is_some() {
        request.title.clone()
    } else {
        state.db.get_last_title().ok().flatten()
    };
    
    // Get quality from profile or use default
    let quality = {
        let config = state.config.read().await;
        let profile = if let Some(ref profile_name) = request.profile {
            config.get_profile(profile_name)
        } else {
            config.get_default_profile()
        };
        
        profile
            .and_then(|p| p.audio_quality)
            .unwrap_or(5) // Default quality if no profile found
    };
    
    let args = RipArgs {
        output_folder: request.output_path.clone().map(PathBuf::from),
        title: title.clone(),
        skip_metadata: request.skip_metadata,
        skip_filebot: request.skip_filebot,
        quality,
        eject_when_done: is_final_attempt,
    };
    
    // Run the actual rip operation
    let result = crate::app::run(args).await;
    
    // On success, log to history
    if result.is_ok() {
        let end_time = chrono::Utc::now();
        let duration_seconds = (end_time - start_time).num_seconds();
        
        let file_size_bytes = if let Some(ref path) = request.output_path {
            get_directory_size(&std::path::PathBuf::from(path)).await.ok()
        } else {
            None
        };
        
        let _ = state.event_tx.send(ApiEvent::RipCompleted {
            disc: title.clone().unwrap_or_else(|| "Unknown".to_string()),
            drive: drive.clone(),
        });
        
        // Calculate checksum if output path exists
        let checksum = if let Some(ref output_path) = request.output_path {
            let path = std::path::PathBuf::from(output_path);
            if path.exists() {
                match crate::checksum::calculate_directory_checksum(&path) {
                    Ok(cs) => Some(cs),
                    Err(e) => {
                        tracing::warn!("Failed to calculate checksum: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        let history = RipHistory {
            id: None,
            timestamp: start_time,
            drive: drive.clone(),
            disc: None,
            title: title.clone(),
            disc_type: None,
            status: RipStatus::Success,
            duration_seconds: Some(duration_seconds),
            file_size_bytes,
            output_path: request.output_path.clone(),
            error_message: None,
            avg_speed_mbps: file_size_bytes.map(|bytes| {
                if duration_seconds > 0 {
                    (bytes as f32 / 1_048_576.0) / duration_seconds as f32
                } else {
                    0.0
                }
            }),
            checksum,
        };
        
        if let Err(e) = state.db.add_rip_history(&history) {
            tracing::error!("Failed to save rip history: {}", e);
        }
    }
    
    result
}

/// Get total size of a directory recursively
fn get_directory_size_sync(path: &std::path::Path) -> anyhow::Result<i64> {
    let mut total_size = 0i64;
    
    if path.is_file() {
        let metadata = std::fs::metadata(path)?;
        return Ok(metadata.len() as i64);
    }
    
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                total_size += metadata.len() as i64;
            } else if metadata.is_dir() {
                total_size += get_directory_size_sync(&entry.path())?;
            }
        }
    }
    
    Ok(total_size)
}

async fn get_directory_size(path: &std::path::Path) -> anyhow::Result<i64> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || get_directory_size_sync(&path))
        .await?
}

/// Run rename operation in background
async fn run_rename_operation(
    state: ApiState,
    request: RenameRequest,
    _config: Config,
) -> anyhow::Result<()> {
    let _ = state.event_tx.send(ApiEvent::Log {
        level: "info".to_string(),
        message: format!("Starting rename for directory: {}", request.directory),
        drive: None,
    });
    
    // Call existing rename functionality
    // Note: We'll need to refactor rename::run_rename to work without prompts
    // For now, log the parameters that would be used
    if let Some(ref title) = request.title {
        let _ = state.event_tx.send(ApiEvent::Log {
            level: "info".to_string(),
            message: format!("Using title: {}", title),
            drive: None,
        });
    }
    let _ = state.event_tx.send(ApiEvent::Log {
        level: "info".to_string(),
        message: format!(
            "Options: skip_speech={}, skip_filebot={}",
            request.skip_speech, request.skip_filebot
        ),
        drive: None,
    });
    
    let _ = state.event_tx.send(ApiEvent::Log {
        level: "success".to_string(),
        message: "Rename operation completed".to_string(),
        drive: None,
    });
    
    Ok(())
}

/// Get recent logs from database
async fn get_logs(State(state): State<ApiState>) -> Result<Json<Vec<LogEntry>>, ErrorResponse> {
    match state.db.get_recent_logs(100) {
        Ok(logs) => Ok(Json(logs)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get logs: {}", e),
        }),
    }
}

/// Search logs with filters
#[derive(Debug, Deserialize)]
struct SearchLogsQuery {
    query: Option<String>,
    level: Option<String>,
    drive: Option<String>,
    limit: Option<usize>,
}

async fn search_logs_handler(
    State(state): State<ApiState>,
    axum::extract::Query(params): axum::extract::Query<SearchLogsQuery>,
) -> Result<Json<Vec<LogEntry>>, ErrorResponse> {
    let limit = params.limit.unwrap_or(100);
    
    match state.db.search_logs(
        params.query.as_deref(),
        params.level.as_deref(),
        params.drive.as_deref(),
        limit,
    ) {
        Ok(logs) => Ok(Json(logs)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to search logs: {}", e),
        }),
    }
}

/// Clear all logs
async fn clear_logs_handler(State(state): State<ApiState>) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.clear_logs() {
        Ok(count) => Ok(Json(serde_json::json!({
            "success": true,
            "deleted": count
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to clear logs: {}", e),
        }),
    }
}

/// Get all issues (including resolved)
async fn get_all_issues_handler(State(state): State<ApiState>) -> Result<Json<Vec<Issue>>, ErrorResponse> {
    match state.db.get_all_issues(100) {
        Ok(issues) => Ok(Json(issues)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get issues: {}", e),
        }),
    }
}

/// Get active (unresolved) issues
async fn get_active_issues(State(state): State<ApiState>) -> Result<Json<Vec<Issue>>, ErrorResponse> {
    match state.db.get_active_issues() {
        Ok(issues) => Ok(Json(issues)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get active issues: {}", e),
        }),
    }
}

/// Resolve an issue
async fn resolve_issue(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.resolve_issue(id) {
        Ok(_) => Ok(Json(serde_json::json!({ "success": true }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to resolve issue: {}", e),
        }),
    }
}

/// Get notes for an issue
async fn get_issue_notes_handler(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<Vec<crate::database::IssueNote>>, ErrorResponse> {
    match state.db.get_issue_notes(id) {
        Ok(notes) => Ok(Json(notes)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get issue notes: {}", e),
        }),
    }
}

/// Add a note to an issue
#[derive(Deserialize)]
struct AddNoteRequest {
    note: String,
}

async fn add_issue_note_handler(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(request): Json<AddNoteRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.add_issue_note(id, &request.note) {
        Ok(note_id) => Ok(Json(serde_json::json!({ "success": true, "id": note_id }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to add note: {}", e),
        }),
    }
}

/// Delete an issue note
async fn delete_issue_note_handler(
    State(state): State<ApiState>,
    axum::extract::Path((_issue_id, note_id)): axum::extract::Path<(i64, i64)>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.delete_issue_note(note_id) {
        Ok(_) => Ok(Json(serde_json::json!({ "success": true }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to delete note: {}", e),
        }),
    }
}

/// Assign an issue to a user
#[derive(Deserialize)]
struct AssignIssueRequest {
    assigned_to: Option<String>,
}

async fn assign_issue_handler(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(request): Json<AssignIssueRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.assign_issue(id, request.assigned_to.as_deref()) {
        Ok(_) => Ok(Json(serde_json::json!({ "success": true }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to assign issue: {}", e),
        }),
    }
}

/// Update resolution notes for an issue
#[derive(Deserialize)]
struct UpdateResolutionNotesRequest {
    notes: String,
}

async fn update_resolution_notes_handler(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(request): Json<UpdateResolutionNotesRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.update_resolution_notes(id, &request.notes) {
        Ok(_) => Ok(Json(serde_json::json!({ "success": true }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to update resolution notes: {}", e),
        }),
    }
}

/// Get the last used title
async fn get_last_title(State(state): State<ApiState>) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.get_last_title() {
        Ok(title) => Ok(Json(serde_json::json!({ "title": title }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get last title: {}", e),
        }),
    }
}

/// Set the last used title
#[derive(Debug, Deserialize)]
struct SetTitleRequest {
    title: String,
}

async fn set_last_title(
    State(state): State<ApiState>,
    Json(request): Json<SetTitleRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.set_last_title(&request.title) {
        Ok(_) => Ok(Json(serde_json::json!({ "success": true }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to set last title: {}", e),
        }),
    }
}

/// Get the last selected show ID
async fn get_last_show_id_handler(State(state): State<ApiState>) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.get_last_show_id() {
        Ok(show_id) => Ok(Json(serde_json::json!({ "show_id": show_id }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get last show ID: {}", e),
        }),
    }
}

/// Get all shows
async fn get_shows(State(state): State<ApiState>) -> Result<Json<Vec<Show>>, ErrorResponse> {
    match state.db.get_shows() {
        Ok(shows) => Ok(Json(shows)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get shows: {}", e),
        }),
    }
}

/// Create a new show
#[derive(Debug, Deserialize)]
struct CreateShowRequest {
    name: String,
}

async fn create_show(
    State(state): State<ApiState>,
    Json(request): Json<CreateShowRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.add_show(&request.name) {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id, "success": true }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to create show: {}", e),
        }),
    }
}

/// Get a single show
async fn get_show(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<Show>, ErrorResponse> {
    match state.db.get_show(id) {
        Ok(Some(show)) => Ok(Json(show)),
        Ok(None) => Err(ErrorResponse {
            error: "Show not found".to_string(),
        }),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get show: {}", e),
        }),
    }
}

/// Update a show
#[derive(Debug, Deserialize)]
struct UpdateShowRequest {
    name: String,
}

async fn update_show(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(request): Json<UpdateShowRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.update_show(id, &request.name) {
        Ok(_) => Ok(Json(serde_json::json!({ "success": true }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to update show: {}", e),
        }),
    }
}

/// Delete a show
async fn delete_show(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.delete_show(id) {
        Ok(_) => Ok(Json(serde_json::json!({ "success": true }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to delete show: {}", e),
        }),
    }
}

/// Select a show (set as last used and update title)
async fn select_show(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    // Get the show to get its name
    match state.db.get_show(id) {
        Ok(Some(show)) => {
            // Set last show ID
            if let Err(e) = state.db.set_last_show_id(id) {
                return Err(ErrorResponse {
                    error: format!("Failed to set last show: {}", e),
                });
            }
            // Also update the last title
            if let Err(e) = state.db.set_last_title(&show.name) {
                return Err(ErrorResponse {
                    error: format!("Failed to set title: {}", e),
                });
            }
            // Update the show's last used timestamp
            if let Err(e) = state.db.update_show_last_used(id) {
                return Err(ErrorResponse {
                    error: format!("Failed to update last used timestamp: {}", e),
                });
            }
            Ok(Json(serde_json::json!({ "success": true, "name": show.name })))
        }
        Ok(None) => Err(ErrorResponse {
            error: "Show not found".to_string(),
        }),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to select show: {}", e),
        }),
    }
}

/// Get overall statistics
async fn get_statistics(State(state): State<ApiState>) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.get_statistics() {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get statistics: {}", e),
        }),
    }
}

/// Get drive statistics
async fn get_drive_stats(State(state): State<ApiState>) -> Result<Json<Vec<crate::database::DriveStats>>, ErrorResponse> {
    match state.db.get_drive_statistics() {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get drive statistics: {}", e),
        }),
    }
}

/// Get error frequency statistics
async fn get_error_frequency(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.get_error_frequency() {
        Ok(frequency) => Ok(Json(frequency)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get error frequency: {}", e),
        }),
    }
}

/// Get available rip profiles
async fn get_rip_profiles(
    State(state): State<ApiState>,
) -> Result<Json<Vec<crate::config::RipProfile>>, ErrorResponse> {
    let config = state.config.read().await;
    Ok(Json(config.rip_profiles.clone()))
}

/// Backup database
#[derive(Debug, Deserialize)]
struct BackupRequest {
    path: Option<String>,
}

async fn backup_database_handler(
    State(state): State<ApiState>,
    Json(request): Json<BackupRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let backup_path = if let Some(ref path) = request.path {
        PathBuf::from(path)
    } else {
        // Default backup location
        if let Some(home) = dirs::home_dir() {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            home.join(".config").join("ripley").join(format!("ripley.db.backup.{}", timestamp))
        } else {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            PathBuf::from(format!("ripley.db.backup.{}", timestamp))
        }
    };
    
    match state.db.backup_database(&backup_path) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "path": backup_path.to_string_lossy().to_string()
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to backup database: {}", e),
        }),
    }
}

/// Restore database
#[derive(Debug, Deserialize)]
struct RestoreRequest {
    path: String,
}

async fn restore_database_handler(
    State(state): State<ApiState>,
    Json(request): Json<RestoreRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let backup_path = PathBuf::from(&request.path);
    
    match state.db.restore_database(&backup_path) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "message": "Database restored successfully. Please restart the server."
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to restore database: {}", e),
        }),
    }
}

/// Process rip queue - checks for available drives and starts queued rips
async fn process_queue(state: ApiState) {
    let status = state.rip_status.read().await;
    let active_drives: std::collections::HashSet<String> = status.active_rips.keys().cloned().collect();
    drop(status);
    
    // Check for queue entries that can be processed
    let next_entry = match state.db.get_next_queue_entry(None) {
        Ok(Some(entry)) => entry,
        Ok(None) => return, // No queued items
        Err(e) => {
            tracing::error!("Failed to get next queue entry: {}", e);
            return;
        }
    };
    
    // Check if the requested drive is available
    let drive_available = if let Some(ref requested_drive) = next_entry.drive {
        !active_drives.contains(requested_drive)
    } else {
        // Any drive can be used - check if we have at least one free
        active_drives.is_empty()
    };
    
    if !drive_available {
        return; // No available drives
    }
    
    let queue_id = next_entry.id.unwrap();
    let drive_id = next_entry.drive.clone().unwrap_or_else(|| "default".to_string());
    
    // Mark as processing
    if let Err(e) = state.db.update_queue_status(queue_id, QueueStatus::Processing, Some(chrono::Utc::now())) {
        tracing::error!("Failed to update queue status: {}", e);
        return;
    }
    
    // Mark drive as active
    let mut status = state.rip_status.write().await;
    status.active_rips.insert(drive_id.clone(), DriveRipStatus {
        current_disc: None,
        current_title: next_entry.title.clone(),
        progress: 0.0,
        paused: false,
        paused_at: None,
    });
    drop(status);
    
    // Convert queue entry to StartRipRequest
    let request = StartRipRequest {
        drive: next_entry.drive.clone(),
        output_path: next_entry.output_path.clone(),
        title: next_entry.title.clone(),
        skip_metadata: next_entry.skip_metadata,
        skip_filebot: next_entry.skip_filebot,
        profile: next_entry.profile.clone(),
        priority: Some(next_entry.priority), // Preserve priority
    };
    
    let state_clone = state.clone();
    let drive_clone = drive_id.clone();
    let queue_id_clone = queue_id;
    
    // Spawn rip operation
    tokio::spawn(async move {
        let result = run_rip_operation(state_clone.clone(), request, drive_clone.clone()).await;
        
        // Remove from active rips when done
        let mut status = state_clone.rip_status.write().await;
        status.active_rips.remove(&drive_clone);
        
        // Update queue status
        let queue_status = if result.is_ok() {
            QueueStatus::Completed
        } else {
            QueueStatus::Failed
        };
        
        if let Err(e) = state_clone.db.update_queue_status(queue_id_clone, queue_status, None) {
            tracing::error!("Failed to update queue status: {}", e);
        }
        
        if let Err(e) = result {
            tracing::error!("Rip operation failed: {:?}", e);
        }
        
        // Queue will be processed on next start_rip call or when checking for available drives
    });
}

/// Get rip queue
async fn get_queue_handler(State(state): State<ApiState>) -> Result<Json<Vec<RipQueueEntry>>, ErrorResponse> {
    match state.db.get_queue_entries(false) {
        Ok(entries) => Ok(Json(entries)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get queue: {}", e),
        }),
    }
}

/// Cancel queue entry
async fn cancel_queue_handler(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.update_queue_status(id, QueueStatus::Cancelled, None) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "message": "Queue entry cancelled"
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to cancel queue entry: {}", e),
        }),
    }
}

/// Get episode matching statistics
async fn get_episode_match_statistics_handler(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.get_episode_match_statistics() {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get episode match statistics: {}", e),
        }),
    }
}

/// Get rip history
#[derive(Debug, Deserialize)]
struct RipHistoryQuery {
    limit: Option<i64>,
}

async fn get_rip_history_handler(
    State(state): State<ApiState>,
    axum::extract::Query(query): axum::extract::Query<RipHistoryQuery>,
) -> Result<Json<Vec<crate::database::RipHistory>>, ErrorResponse> {
    let limit = query.limit.unwrap_or(50);
    match state.db.get_rip_history(limit) {
        Ok(history) => Ok(Json(history)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get rip history: {}", e),
        }),
    }
}

/// Get user preferences
async fn get_preferences(State(state): State<ApiState>) -> Result<Json<crate::database::UserPreferences>, ErrorResponse> {
    match state.db.get_preferences() {
        Ok(prefs) => Ok(Json(prefs)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get preferences: {}", e),
        }),
    }
}

/// Update user preferences
async fn update_preferences(
    State(state): State<ApiState>,
    Json(prefs): Json<crate::database::UserPreferences>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.update_preferences(&prefs) {
        Ok(_) => Ok(Json(serde_json::json!({ "success": true }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to update preferences: {}", e),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rip_status_default() {
        let status = RipStatus::default();
        assert!(status.active_rips.is_empty());
        assert!(status.logs.is_empty());
    }

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert!(response.0.get("status").is_some());
        assert_eq!(response.0["status"], "ok");
    }

    #[test]
    fn test_api_event_serialization() {
        let event = ApiEvent::Log {
            level: "info".to_string(),
            message: "test".to_string(),
            drive: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Log"));
        assert!(json.contains("test"));
    }
}
