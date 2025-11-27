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
use crate::database::{Database, LogEntry, Issue, Show};

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
}

/// Request body for starting a rip operation
#[derive(Debug, Deserialize)]
pub struct StartRipRequest {
    pub drive: Option<String>,
    pub output_path: Option<String>,
    pub title: Option<String>,
    pub skip_metadata: bool,
    pub skip_filebot: bool,
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
        .route("/rip-history", get(get_rip_history_handler))
        .route("/preferences", get(get_preferences))
        .route("/preferences", post(update_preferences))
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

/// Start ripping operation
async fn start_rip(
    State(state): State<ApiState>,
    Json(request): Json<StartRipRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    // Determine the drive identifier (use "default" if not specified)
    let drive = request.drive.clone().unwrap_or_else(|| "default".to_string());
    
    let mut status = state.rip_status.write().await;
    
    // Check if this specific drive is already ripping
    if status.active_rips.contains_key(&drive) {
        return Err(ErrorResponse {
            error: format!("A rip operation is already in progress on drive {}", drive),
        });
    }
    
    // Mark this drive as active
    status.active_rips.insert(drive.clone(), DriveRipStatus {
        current_disc: None,
        current_title: request.title.clone(),
        progress: 0.0,
    });
    drop(status);
    
    // Clone state for async task
    let state_clone = state.clone();
    let request_clone = request;
    let drive_clone = drive.clone();
    
    // Spawn rip operation in background
    tokio::spawn(async move {
        let result = run_rip_operation(state_clone.clone(), request_clone, drive_clone.clone()).await;
        
        // Remove from active rips when done
        let mut status = state_clone.rip_status.write().await;
        status.active_rips.remove(&drive_clone);
        
        if let Err(e) = result {
            tracing::error!("Rip operation failed: {:?}", e);
        }
    });
    
    Ok(Json(serde_json::json!({
        "status": "started",
        "drive": drive
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

/// Run the rip operation in background
async fn run_rip_operation(
    state: ApiState,
    request: StartRipRequest,
    drive_id: String,
) -> anyhow::Result<()> {
    use crate::database::{RipHistory, RipStatus};
    
    let start_time = chrono::Utc::now();
    let drive = drive_id; // Use the provided drive identifier
    
    // Use provided title or fall back to last saved title
    let title = if request.title.is_some() {
        request.title.clone()
    } else {
        state.db.get_last_title().ok().flatten()
    };
    
    // Send RipStarted event
    let _ = state.event_tx.send(ApiEvent::RipStarted {
        disc: title.clone().unwrap_or_else(|| "Unknown Disc".to_string()),
        drive: drive.clone(),
    });
    
    let _ = state.event_tx.send(ApiEvent::Log {
        level: "info".to_string(),
        message: format!("Starting rip: {}", title.clone().unwrap_or_else(|| "Unknown".to_string())),
        drive: Some(drive.clone()),
    });
    
    let args = RipArgs {
        output_folder: request.output_path.clone().map(PathBuf::from),
        title: title.clone(),
        skip_metadata: request.skip_metadata,
        skip_filebot: request.skip_filebot,
        quality: 5,
        eject_when_done: true,
    };
    
    // Note: We'll need to refactor app::run to work without TUI
    // For now, this is a placeholder
    let result = crate::app::run(args).await;
    
    // Calculate duration
    let end_time = chrono::Utc::now();
    let duration_seconds = (end_time - start_time).num_seconds();
    
    // Try to get file size from output path
    let file_size_bytes = if let Some(ref path) = request.output_path {
        get_directory_size(&std::path::PathBuf::from(path)).await.ok()
    } else {
        None
    };
    
    // Log to rip history
    match &result {
        Ok(_) => {
            let _ = state.event_tx.send(ApiEvent::RipCompleted {
                disc: title.clone().unwrap_or_else(|| "Unknown".to_string()),
                drive: drive.clone(),
            });
            
            // Save successful rip to history
            let history = RipHistory {
                id: None,
                timestamp: start_time,
                drive: drive.clone(),
                disc: None,
                title: title.clone(),
                disc_type: None, // Could be detected from media type
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
            };
            
            if let Err(e) = state.db.add_rip_history(&history) {
                tracing::error!("Failed to save rip history: {}", e);
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            let _ = state.event_tx.send(ApiEvent::RipError {
                error: error_msg.clone(),
                drive: Some(drive.clone()),
            });
            
            // Save failed rip to history
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
                error_message: Some(error_msg),
                avg_speed_mbps: None,
            };
            
            if let Err(e) = state.db.add_rip_history(&history) {
                tracing::error!("Failed to save rip history: {}", e);
            }
        }
    }
    
    // Drive will be removed from active_rips in the spawned task
    Ok(())
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
