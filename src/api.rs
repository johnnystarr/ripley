use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        multipart::Multipart,
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
use crate::database::{Database, LogEntry, Issue, Show, RipQueueEntry, QueueStatus, AgentInfo, TopazProfile, UpscalingJob, JobStatus};

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
        operations: Arc::new(RwLock::new(std::collections::HashMap::new())),
    };
    
    // Spawn background task to log events to database
    let mut event_rx = event_tx.subscribe();
    let db_logger = Arc::clone(&db);
    tokio::spawn(async move {
        use crate::database::{LogEntry, LogLevel, Issue, IssueType};
        
        while let Ok(event) = event_rx.recv().await {
            match event {
                ApiEvent::Log { level, message, drive, operation_id: _ } => {
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
                ApiEvent::RipError { error, drive, operation_id: _ } => {
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
    
    // Spawn background task to cleanup stale agents (every 2 minutes for better responsiveness)
    let db_cleanup = Arc::clone(&db);
    let event_tx_cleanup = event_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(120)); // 2 minutes
        loop {
            interval.tick().await;
            match db_cleanup.cleanup_stale_agents(2) { // Mark offline if no heartbeat in 2 minutes
                Ok(count) => {
                    if count > 0 {
                        info!("Marked {} stale agent(s) as offline", count);
                        // Get list of agents that were marked offline and broadcast status changes
                        if let Ok(agents) = db_cleanup.get_agents() {
                            for agent in agents {
                                if agent.status == "offline" {
                                    let _ = event_tx_cleanup.send(ApiEvent::AgentStatusChanged {
                                        agent_id: agent.agent_id.clone(),
                                        status: "offline".to_string(),
                                        last_seen: agent.last_seen.clone(),
                                        operation_id: None,
                                    });
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to cleanup stale agents: {}", e);
                }
            }
        }
    });
    
    // Spawn background task to poll for drive changes
    let event_tx_poller = event_tx.clone();
    let state_for_poller = state.clone();
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
                    
                    // Find newly detected drives or drives with newly inserted media
                    for (device, drive_info) in &current_map {
                        let was_known = known_drives.contains_key(device);
                        let had_media = known_drives.get(device)
                            .map(|d| !matches!(d.media_type, crate::drive::MediaType::None))
                            .unwrap_or(false);
                        let has_media = !matches!(drive_info.media_type, crate::drive::MediaType::None);
                        
                        if !was_known {
                            info!("Drive detected: {}", device);
                            let _ = event_tx_poller.send(ApiEvent::DriveDetected {
                                drive: drive_info.clone(),
                            });
                        } else if !had_media && has_media {
                            info!("Media inserted in drive: {} (type: {:?})", device, drive_info.media_type);
                            let _ = event_tx_poller.send(ApiEvent::DriveDetected {
                                drive: drive_info.clone(),
                            });
                        }
                        
                        // Auto-start rip if media is detected and drive is not already ripping
                        if has_media && (!was_known || !had_media) {
                            let state_for_rip = state_for_poller.clone();
                            let device_for_rip = device.clone();
                            
                            // Check if already ripping this drive
                            let is_ripping = {
                                let status = state_for_poller.rip_status.read().await;
                                status.active_rips.contains_key(device)
                            };
                            
                            if !is_ripping {
                                info!("Auto-starting rip for drive {} with media type {:?}", device, drive_info.media_type);
                                let state_clone = state_for_rip.clone();
                                tokio::spawn(async move {
                                    // Small delay to ensure drive is ready
                                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                    
                                    let rip_request = StartRipRequest {
                                        drive: Some(device_for_rip.clone()),
                                        output_path: None,
                                        title: None,
                                        skip_metadata: false,
                                        skip_filebot: false,
                                        profile: None,
                                        priority: None,
                                    };
                                    
                                    // Call the internal start_rip logic
                                    match start_rip_internal(&state_clone, &rip_request, &device_for_rip).await {
                                        Ok(_) => {
                                            info!("Successfully auto-started rip for {}", device_for_rip);
                                        }
                                        Err(e) => {
                                            tracing::warn!("Failed to auto-start rip for {}: {}", device_for_rip, e);
                                        }
                                    }
                                });
                            }
                        }
                    }
                    
                    // Find removed drives
                    for device in known_drives.keys() {
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
    pub operations: Arc<RwLock<std::collections::HashMap<String, Operation>>>,
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

/// Operation type enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Rip,
    Upscale,
    Rename,
    Transfer,
    Other,
}

/// Operation status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

/// Active operation tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub operation_id: String,
    pub operation_type: OperationType,
    pub status: OperationStatus,
    pub drive: Option<String>,
    pub title: Option<String>,
    pub progress: f32,
    pub message: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
}

/// Generate a unique operation ID
fn generate_operation_id(operation_type: OperationType, drive: Option<&String>) -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let drive_suffix = drive.map(|d| d.replace("/", "_")).unwrap_or_else(|| "default".to_string());
    format!("{}_{}_{}", 
        match operation_type {
            OperationType::Rip => "rip",
            OperationType::Upscale => "upscale",
            OperationType::Rename => "rename",
            OperationType::Transfer => "transfer",
            OperationType::Other => "op",
        },
        drive_suffix,
        timestamp
    )
}

/// Create and register a new operation
async fn create_operation(
    state: &ApiState,
    operation_type: OperationType,
    drive: Option<String>,
    title: Option<String>,
    initial_message: String,
) -> String {
    let operation_id = generate_operation_id(operation_type, drive.as_ref());
    let operation = Operation {
        operation_id: operation_id.clone(),
        operation_type,
        status: OperationStatus::Running,
        drive,
        title,
        progress: 0.0,
        message: initial_message,
        started_at: chrono::Utc::now(),
        completed_at: None,
        error: None,
    };
    
    // Add to operations map
    {
        let mut operations = state.operations.write().await;
        operations.insert(operation_id.clone(), operation.clone());
    }
    
    // Broadcast operation started event
    let _ = state.event_tx.send(ApiEvent::OperationStarted {
        operation: operation.clone(),
    });
    
    operation_id
}

/// Update an operation's progress
async fn update_operation(
    state: &ApiState,
    operation_id: &str,
    progress: f32,
    message: String,
) {
    let mut operations = state.operations.write().await;
    if let Some(op) = operations.get_mut(operation_id) {
        op.progress = progress;
        op.message = message.clone();
        
        // Broadcast progress update
        drop(operations);
        let _ = state.event_tx.send(ApiEvent::OperationProgress {
            operation_id: operation_id.to_string(),
            progress,
            message,
        });
    }
}

/// Complete an operation (success)
async fn complete_operation(
    state: &ApiState,
    operation_id: &str,
    final_message: Option<String>,
) {
    let mut operations = state.operations.write().await;
    if let Some(op) = operations.get_mut(operation_id) {
        op.status = OperationStatus::Completed;
        op.completed_at = Some(chrono::Utc::now());
        op.progress = 100.0;
        if let Some(msg) = final_message {
            op.message = msg;
        }
        
        // Clone operation data for saving to database
        let op_for_db = op.clone();
        let operation_id_str = operation_id.to_string();
        drop(operations);
        
        // Save to operation history
        let operation_type_str = match op_for_db.operation_type {
            OperationType::Rip => "rip",
            OperationType::Upscale => "upscale",
            OperationType::Rename => "rename",
            OperationType::Transfer => "transfer",
            OperationType::Other => "other",
        };
        let status_str = "completed";
        let started_at_str = op_for_db.started_at.to_rfc3339();
        let completed_at_str = op_for_db.completed_at.map(|dt| dt.to_rfc3339());
        
        if let Err(e) = state.db.save_operation_to_history(
            &operation_id_str,
            operation_type_str,
            status_str,
            op_for_db.drive.as_deref(),
            op_for_db.title.as_deref(),
            op_for_db.progress,
            &op_for_db.message,
            &started_at_str,
            completed_at_str.as_deref(),
            None,
        ) {
            tracing::warn!("Failed to save operation to history: {}", e);
        }
        
        // Broadcast completion event
        let _ = state.event_tx.send(ApiEvent::OperationCompleted {
            operation_id: operation_id_str.clone(),
        });
        
        // Remove from active operations after a delay (to allow clients to see completion)
        // For now, we'll keep completed operations until they're manually cleaned up
    }
}

/// Fail an operation
async fn fail_operation(
    state: &ApiState,
    operation_id: &str,
    error: String,
) {
    let mut operations = state.operations.write().await;
    if let Some(op) = operations.get_mut(operation_id) {
        op.status = OperationStatus::Failed;
        op.completed_at = Some(chrono::Utc::now());
        op.error = Some(error.clone());
        
        // Clone operation data for saving to database
        let op_for_db = op.clone();
        let operation_id_str = operation_id.to_string();
        let error_str = error.clone();
        drop(operations);
        
        // Save to operation history
        let operation_type_str = match op_for_db.operation_type {
            OperationType::Rip => "rip",
            OperationType::Upscale => "upscale",
            OperationType::Rename => "rename",
            OperationType::Transfer => "transfer",
            OperationType::Other => "other",
        };
        let status_str = "failed";
        let started_at_str = op_for_db.started_at.to_rfc3339();
        let completed_at_str = op_for_db.completed_at.map(|dt| dt.to_rfc3339());
        
        if let Err(e) = state.db.save_operation_to_history(
            &operation_id_str,
            operation_type_str,
            status_str,
            op_for_db.drive.as_deref(),
            op_for_db.title.as_deref(),
            op_for_db.progress,
            &op_for_db.message,
            &started_at_str,
            completed_at_str.as_deref(),
            Some(&error_str),
        ) {
            tracing::warn!("Failed to save failed operation to history: {}", e);
        }
        
        // Broadcast failure event
        let _ = state.event_tx.send(ApiEvent::OperationFailed {
            operation_id: operation_id_str.clone(),
            error,
        });
    }
}

/// Current ripping status (supports multiple drives)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct RipStatus {
    pub active_rips: std::collections::HashMap<String, DriveRipStatus>,
    pub logs: Vec<String>,
}

/// Events broadcast to WebSocket clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ApiEvent {
    RipStarted { disc: String, drive: String, operation_id: Option<String> },
    RipProgress { progress: f32, message: String, drive: String, operation_id: Option<String> },
    RipCompleted { disc: String, drive: String, operation_id: Option<String> },
    RipError { error: String, drive: Option<String>, operation_id: Option<String> },
    Log { level: String, message: String, drive: Option<String>, operation_id: Option<String> },
    StatusUpdate { status: RipStatus },
    DriveDetected { drive: crate::drive::DriveInfo },
    DriveRemoved { device: String },
    DriveEjected { device: String },
    IssueCreated { issue: Issue },
    RipPaused { drive: String, operation_id: Option<String> },
    RipResumed { drive: String, operation_id: Option<String> },
    OperationStarted { operation: Operation },
    OperationProgress { operation_id: String, progress: f32, message: String },
    OperationCompleted { operation_id: String },
    OperationFailed { operation_id: String, error: String },
    AgentStatusChanged { agent_id: String, status: String, last_seen: String, operation_id: Option<String> },
    UpscalingJobStatusChanged { job_id: String, status: String, progress: f32, error_message: Option<String>, operation_id: Option<String> },
}

/// Request body for starting a rip operation
#[derive(Debug, Clone, Deserialize)]
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
        .route("/config/database/reset", post(reset_database_handler))
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
        .route("/monitor/operations", get(get_monitor_operations))
        .route("/monitor/operations/history", get(get_operation_history))
        .route("/monitor/drives", get(get_monitor_drives))
        // Agent endpoints
        .route("/agents", get(get_agents))
        .route("/agents/register", post(register_agent))
        .route("/agents/:agent_id/heartbeat", post(agent_heartbeat))
        .route("/agents/:agent_id/instructions", get(get_agent_instructions))
        .route("/agents/:agent_id/output-location", get(get_agent_output_location))
        .route("/agents/:agent_id/output-location", put(update_agent_output_location))
        .route("/agents/:agent_id/disconnect", post(disconnect_agent))
        .route("/agents/:agent_id", delete(delete_agent))
        .route("/agents/:agent_id/test", post(test_agent_command))
        .route("/agents/instructions", post(create_instruction))
        .route("/agents/instructions/:id/assign", post(assign_instruction))
        .route("/agents/instructions/:id/start", post(start_instruction))
        .route("/agents/instructions/:id/complete", post(complete_instruction))
        .route("/agents/instructions/:id/fail", post(fail_instruction))
        .route("/agents/instructions/:id", get(get_instruction))
        .route("/agents/upload", post(upload_file))
        .route("/agents/download/:file_id", get(download_file))
        // Topaz Profile endpoints
        .route("/topaz-profiles", get(get_topaz_profiles))
        .route("/topaz-profiles", post(create_topaz_profile))
        .route("/topaz-profiles/:id", get(get_topaz_profile))
        .route("/topaz-profiles/:id", put(update_topaz_profile))
        .route("/topaz-profiles/:id", delete(delete_topaz_profile))
        .route("/topaz-profiles/:id/shows/:show_id", post(associate_profile_with_show))
        .route("/topaz-profiles/:id/shows/:show_id", delete(remove_profile_from_show))
        .route("/shows/:show_id/topaz-profiles", get(get_profiles_for_show))
        // Upscaling Job endpoints
        .route("/upscaling-jobs", get(get_upscaling_jobs))
        .route("/upscaling-jobs", post(create_upscaling_job))
        .route("/upscaling-jobs/next", get(get_next_upscaling_job))
        .route("/upscaling-jobs/:job_id/assign", post(assign_upscaling_job))
        .route("/upscaling-jobs/:job_id/status", put(update_upscaling_job_status))
        .route("/upscaling-jobs/:job_id/output", put(update_upscaling_job_output))
        .route("/upscaling-jobs/:job_id/retry", post(retry_upscaling_job))
        .route("/upscaling-jobs/cleanup", post(cleanup_old_upscaling_jobs))
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

/// Reset database handler
async fn reset_database_handler(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.reset_database() {
        Ok(_) => {
            info!("Database reset by user");
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Database reset successfully. All data deleted and schema reinitialized."
            })))
        }
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to reset database: {}", e),
        }),
    }
}

/// Internal function to start a rip operation (used by both HTTP handler and auto-rip)
async fn start_rip_internal(
    state: &ApiState,
    request: &StartRipRequest,
    drive_id: &str,
) -> Result<(), String> {
    let mut status = state.rip_status.write().await;
    
    // Check if this drive is already ripping
    if status.active_rips.contains_key(drive_id) {
        drop(status);
        return Err(format!("Drive {} is already ripping", drive_id));
    }
    
    // Create operation for this rip
    let operation_id = create_operation(
        state,
        OperationType::Rip,
        Some(drive_id.to_string()),
        request.title.clone(),
        format!("Starting rip on drive {}", drive_id),
    ).await;
    
    // Mark this drive as active
    status.active_rips.insert(drive_id.to_string(), DriveRipStatus {
        current_disc: None,
        current_title: request.title.clone(),
        progress: 0.0,
        paused: false,
        paused_at: None,
    });
    drop(status);
    
    // Clone state for async task
    let state_clone = state.clone();
    let request_clone = request.clone();
    let output_path_clone = request_clone.output_path.clone();
    let title_clone = request_clone.title.clone();
    let drive_clone = drive_id.to_string();
    let operation_id_clone = operation_id.clone();
    
    // Spawn rip operation in background
    tokio::spawn(async move {
        let result = run_rip_operation(state_clone.clone(), request_clone, drive_clone.clone(), operation_id_clone.clone()).await;
        
        // Remove from active rips when done
        let mut status = state_clone.rip_status.write().await;
        status.active_rips.remove(&drive_clone);
        
        // Complete or fail the operation
        if let Err(ref e) = result {
            tracing::error!("Rip operation failed: {:?}", e);
            fail_operation(&state_clone, &operation_id_clone, format!("{}", e)).await;
        } else {
            // Try to create upscaling job if this was a video rip
            if let Some(ref output_path) = output_path_clone {
                if let Err(e) = create_upscaling_job_for_rip(&state_clone, output_path, title_clone.as_deref()).await {
                    tracing::warn!("Failed to create upscaling job after rip: {}", e);
                }
            }
            
            complete_operation(&state_clone, &operation_id_clone, Some("Rip completed successfully".to_string())).await;
        }
        
        // Process queue after rip completes
    });
    
    Ok(())
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
    
    match start_rip_internal(&state, &request, &drive_id).await {
        Ok(_) => {
            Ok(Json(serde_json::json!({
                "status": "started",
                "drive": drive_id
            })))
        }
        Err(e) => {
            Err(ErrorResponse {
                error: format!("Failed to start rip: {}", e),
            })
        }
    }
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
        operation_id: None,
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
            operation_id: None,
        });
        let _ = state.event_tx.send(ApiEvent::RipPaused {
            drive: drive.to_string(),
            operation_id: None,
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
            operation_id: None,
        });
        let _ = state.event_tx.send(ApiEvent::RipResumed {
            drive: drive.to_string(),
            operation_id: None,
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
                operation_id: None,
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
    operation_id: String,
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
                operation_id: Some(operation_id.to_string()),
            });
            
            tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
        }
        
        let is_final_attempt = attempt >= max_attempts;
        update_operation(&state, &operation_id, 10.0 * attempt as f32, format!("Rip attempt {}/{}", attempt, max_attempts)).await;
        
        match run_single_rip_attempt(&state, &request, &drive, start_time, is_final_attempt, &operation_id).await {
            Ok(_) => {
                // Success - return immediately (history already logged)
                update_operation(&state, &operation_id, 100.0, "Rip completed successfully".to_string()).await;
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
        operation_id: Some(operation_id.clone()),
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
    drive: &str,
    start_time: chrono::DateTime<chrono::Utc>,
    is_final_attempt: bool,
    operation_id: &str,
) -> anyhow::Result<()> {
    use crate::database::{RipHistory, RipStatus};
    
    // Get title: use provided, or selected show name, or last saved title
    let title = if request.title.is_some() {
        request.title.clone()
    } else {
        // Try to get selected show name from database
        if let Ok(Some(show_id)) = state.db.get_last_show_id() {
            if let Ok(Some(show)) = state.db.get_show(show_id) {
                info!("Using selected show: {}", show.name);
                // Send log after we have the title
                Some(show.name)
            } else {
                state.db.get_last_title().ok().flatten()
            }
        } else {
            state.db.get_last_title().ok().flatten()
        }
    };
    
    // Detect media type - get from current drives
    let media_type = {
        match crate::drive::detect_drives().await {
            Ok(drives) => {
                drives.iter()
                    .find(|d| d.device == drive)
                    .map(|d| d.media_type.clone())
                    .unwrap_or(crate::drive::MediaType::None)
            }
            Err(_) => crate::drive::MediaType::None,
        }
    };
    
    // Run web-UI-only rip operation (no TUI)
    let result = rip_disc_web_ui(
        state,
        drive,
        media_type,
        title.clone(),
        request,
        operation_id,
        is_final_attempt,
    ).await;
    
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
            drive: drive.to_string(),
            operation_id: Some(operation_id.to_string()),
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
            drive: drive.to_string(),
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

// Helper functions for web UI logging (defined at module level so they can be shared)
async fn send_log_to_web_ui(
    state: &ApiState,
    drive: &str,
    level: &str,
    message: String,
    operation_id: Option<&str>,
) {
    let _ = state.event_tx.send(ApiEvent::Log {
        level: level.to_string(),
        message: message.clone(),
        drive: Some(drive.to_string()),
        operation_id: operation_id.map(|s| s.to_string()),
    });
    
    // Also log to database
    use crate::database::{LogEntry, LogLevel};
    let log_level = match level {
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
        drive: Some(drive.to_string()),
        disc: None,
        title: None,
        context: None,
    };
    
    let _ = state.db.add_log(&entry);
}

async fn update_operation_progress(
    state: &ApiState,
    operation_id: &str,
    progress: f32,
    message: String,
) {
    update_operation(state, operation_id, progress, message.clone()).await;
    let _ = state.event_tx.send(ApiEvent::OperationProgress {
        operation_id: operation_id.to_string(),
        progress,
        message,
    });
}

/// Web-UI-only rip function (no TUI, all logs go to web UI)
async fn rip_disc_web_ui(
    state: &ApiState,
    device: &str,
    media_type: crate::drive::MediaType,
    title: Option<String>,
    request: &StartRipRequest,
    operation_id: &str,
    eject_when_done: bool,
) -> anyhow::Result<()> {
    // Log that we're using the selected show if we have a title
    if let Some(ref show_name) = title {
        send_log_to_web_ui(state, device, "info", format!("📺 Using selected show: {}", show_name), Some(operation_id)).await;
    }
    
    // Handle DVD/Blu-ray ripping
    if matches!(media_type, crate::drive::MediaType::DVD | crate::drive::MediaType::BluRay) {
        return rip_dvd_disc_web_ui(state, device, media_type, title, request, operation_id, eject_when_done).await;
    }
    
    // Handle audio CD ripping
    send_log_to_web_ui(state, device, "info", format!("📀 Detected audio CD in {}", device), Some(operation_id)).await;
    send_log_to_web_ui(state, device, "info", "💿 Preparing disc for reading...".to_string(), Some(operation_id)).await;
    
    // Unmount disc before reading
    for attempt in 1..=3 {
        match crate::drive::unmount_disc(device).await {
            Ok(_) => {
                info!("Unmounted {} for disc ID reading (attempt {})", device, attempt);
                break;
            }
            Err(e) => {
                tracing::warn!("Unmount attempt {}: {}", attempt, e);
                if attempt < 3 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    // Fetch metadata
    send_log_to_web_ui(state, device, "info", format!("🔍 Fetching metadata for {}...", device), Some(operation_id)).await;
    update_operation_progress(state, operation_id, 5.0, "Fetching disc metadata...".to_string()).await;
    
    let disc_id = match crate::metadata::get_disc_id(device).await {
        Ok(id) => {
            send_log_to_web_ui(state, device, "info", format!("📀 Disc ID: {}", id), Some(operation_id)).await;
            id
        }
        Err(e) => {
            send_log_to_web_ui(state, device, "warning", format!("⚠️  Could not get disc ID: {}", e), Some(operation_id)).await;
            if request.skip_metadata {
                "unknown".to_string()
            } else {
                return Err(e);
            }
        }
    };
    
    let metadata = if request.skip_metadata {
        create_dummy_metadata()
    } else {
        match crate::metadata::fetch_metadata(&disc_id, 3).await {
            Ok(meta) => {
                send_log_to_web_ui(state, device, "success", format!("✅ Found: {} - {} ({} tracks)", 
                    meta.artist, meta.album, meta.tracks.len()), Some(operation_id)).await;
                meta
            }
            Err(e) => {
                send_log_to_web_ui(state, device, "warning", format!("⚠️  Metadata lookup failed: {}", e), Some(operation_id)).await;
                send_log_to_web_ui(state, device, "info", "Using generic track names. You can rename files after ripping.".to_string(), Some(operation_id)).await;
                create_dummy_metadata()
            }
        }
    };
    
    let album_info = format!("{} - {}", metadata.artist, metadata.album);
    send_log_to_web_ui(state, device, "info", format!("🎵 Ripping {} from {}...", album_info, device), Some(operation_id)).await;
    update_operation_progress(state, operation_id, 10.0, format!("Ripping: {}", album_info)).await;
    
    let output_folder = if let Some(ref path) = request.output_path {
        std::path::PathBuf::from(path)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home).join("Desktop").join("Rips").join("Music")
    };
    
    if !output_folder.exists() {
        tokio::fs::create_dir_all(&output_folder).await?;
    }
    
    let device_progress = device.to_string();
    let state_progress = state.clone();
    let operation_id_progress = operation_id.to_string();
    let device_log = device.to_string();
    let state_log = state.clone();
    let operation_id_log = operation_id.to_string();
    
    let result = crate::ripper::rip_cd(
        device,
        &metadata,
        &output_folder,
        5, // Default quality
        move |progress| {
            let state = state_progress.clone();
            let operation_id = operation_id_progress.clone();
            
            tokio::spawn(async move {
                let progress_pct = progress.percentage;
                let message = format!("Track {}/{}: {} ({:.1}%)", 
                    progress.current_track, 
                    progress.total_tracks,
                    progress.track_name,
                    progress_pct);
                update_operation_progress(&state, &operation_id, progress_pct, message).await;
            });
        },
        move |log_line| {
            let state = state_log.clone();
            let operation_id = operation_id_log.clone();
            let device = device_log.clone();
            
            tokio::spawn(async move {
                send_log_to_web_ui(&state, &device, "info", log_line, Some(&operation_id)).await;
            });
        },
    ).await;
    
    match result {
        Ok(_) => {
            send_log_to_web_ui(state, device, "success", format!("✅ Completed: {}", album_info), Some(operation_id)).await;
            update_operation_progress(state, operation_id, 100.0, "Rip completed successfully".to_string()).await;
            
            // Filebot music processing if enabled
            let config = state.config.read().await;
            if config.filebot.use_for_music {
                send_log_to_web_ui(state, device, "info", "🎵 Running Filebot to standardize filenames...".to_string(), Some(operation_id)).await;
                
                let album_dir = output_folder
                    .join(crate::ripper::sanitize_filename(&metadata.artist))
                    .join(crate::ripper::sanitize_filename(&metadata.album));
                
                let state_clone = state.clone();
                let device_clone = device.to_string();
                let operation_id_clone = operation_id.to_string();
                
                if let Err(e) = crate::filebot::rename_music_with_filebot(
                    &album_dir,
                    move |log_msg| {
                        let state = state_clone.clone();
                        let device = device_clone.clone();
                        let operation_id = operation_id_clone.clone();
                        tokio::spawn(async move {
                            send_log_to_web_ui(&state, &device, "info", log_msg, Some(&operation_id)).await;
                        });
                    }
                ).await {
                    tracing::warn!("Filebot music processing failed: {}", e);
                    send_log_to_web_ui(state, device, "warning", format!("⚠️  Filebot failed: {}", e), Some(operation_id)).await;
                }
            }
            
            // Send notification
            let disc_info = crate::notifications::DiscInfo {
                disc_type: crate::notifications::DiscType::CD,
                title: album_info.clone(),
                device: device.to_string(),
            };
            if let Err(e) = crate::notifications::send_completion_notification(disc_info, true).await {
                tracing::warn!("Failed to send notification: {}", e);
            }
            
            // Always eject disc when done
            match crate::drive::eject_disc(device).await {
                Ok(_) => {
                    send_log_to_web_ui(state, device, "info", format!("⏏️  Ejected {}", device), Some(operation_id)).await;
                }
                Err(e) => {
                    send_log_to_web_ui(state, device, "warning", format!("⚠️  Failed to eject {}: {}", device, e), Some(operation_id)).await;
                }
            }
        }
        Err(e) => {
            send_log_to_web_ui(state, device, "error", format!("❌ Failed: {} - {}", album_info, e), Some(operation_id)).await;
            update_operation_progress(state, operation_id, 0.0, format!("Rip failed: {}", e)).await;
            
            // Send failure notification
            let disc_info = crate::notifications::DiscInfo {
                disc_type: crate::notifications::DiscType::CD,
                title: album_info.clone(),
                device: device.to_string(),
            };
            if let Err(e) = crate::notifications::send_completion_notification(disc_info, false).await {
                tracing::warn!("Failed to send notification: {}", e);
            }
            return Err(e);
        }
    }
    
    Ok(())
}

/// Web-UI-only DVD/Blu-ray rip function (no TUI, all logs go to web UI)
async fn rip_dvd_disc_web_ui(
    state: &ApiState,
    device: &str,
    media_type: crate::drive::MediaType,
    title: Option<String>,
    request: &StartRipRequest,
    operation_id: &str,
    eject_when_done: bool,
) -> anyhow::Result<()> {
    let media_name = match media_type {
        crate::drive::MediaType::BluRay => "Blu-ray",
        crate::drive::MediaType::DVD => "DVD",
        _ => "disc",
    };
    
    send_log_to_web_ui(state, device, "info", format!("📀 Detected {} in {}", media_name, device), Some(operation_id)).await;
    update_operation_progress(state, operation_id, 1.0, format!("Detected {} in {}", media_name, device)).await;
    
    // Try to get disc volume name
    send_log_to_web_ui(state, device, "info", format!("🔍 Fetching {} metadata...", media_name), Some(operation_id)).await;
    update_operation_progress(state, operation_id, 2.0, "Fetching disc metadata...".to_string()).await;
    
    let volume_name = match get_dvd_volume_name(device).await {
        Ok(name) => {
            send_log_to_web_ui(state, device, "info", format!("💿 Volume name: {}", name), Some(operation_id)).await;
            Some(name)
        }
        Err(e) => {
            send_log_to_web_ui(state, device, "warning", format!("⚠️  Could not get volume name: {}", e), Some(operation_id)).await;
            None
        }
    };
    
    // Use provided title, or selected show name, or volume name
    let title_to_search = if !request.skip_metadata {
        let final_title = title
            .or_else(|| volume_name.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        
        if final_title != "Unknown" {
            send_log_to_web_ui(state, device, "info", format!("📺 Using title: '{}'", final_title), Some(operation_id)).await;
            Some(final_title)
        } else {
            send_log_to_web_ui(state, device, "warning", "❌ No title available, skipping metadata".to_string(), Some(operation_id)).await;
            None
        }
    } else {
        send_log_to_web_ui(state, device, "info", "⏭️  Skipping metadata (--skip-metadata)".to_string(), Some(operation_id)).await;
        None
    };
    
    let dvd_metadata = if let Some(ref title_str) = title_to_search {
        send_log_to_web_ui(state, device, "info", format!("🔍 Searching TMDB for '{}'...", title_str), Some(operation_id)).await;
        update_operation_progress(state, operation_id, 3.0, format!("Searching TMDB for '{}'...", title_str)).await;
        
        match crate::dvd_metadata::fetch_dvd_metadata("", Some(title_str)).await {
            Ok(meta) => {
                send_log_to_web_ui(state, device, "success", format!("📺 Found: {}", meta.title), Some(operation_id)).await;
                if meta.media_type == crate::dvd_metadata::MediaType::TVShow && !meta.episodes.is_empty() {
                    send_log_to_web_ui(state, device, "info", format!("📝 {} episodes available for matching", meta.episodes.len()), Some(operation_id)).await;
                }
                Some(meta)
            }
            Err(e) => {
                send_log_to_web_ui(state, device, "warning", format!("⚠️  Could not fetch metadata: {}", e), Some(operation_id)).await;
                None
            }
        }
    } else {
        None
    };
    
    let default_label = match media_type {
        crate::drive::MediaType::BluRay => "Blu-ray Video",
        crate::drive::MediaType::DVD => "DVD Video",
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
    
    // Determine output directory
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let base_rips = std::path::PathBuf::from(home).join("Desktop").join("Rips");
    
    let media_output = match media_type {
        crate::drive::MediaType::BluRay => base_rips.join("BluRays"),
        crate::drive::MediaType::DVD => base_rips.join("DVDs"),
        _ => base_rips.join("Videos"),
    };
    
    // Create output folder with title or timestamp
    let folder_name = if let Some(ref meta) = dvd_metadata {
        crate::ripper::sanitize_filename(&meta.title)
    } else {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let prefix = match media_type {
            crate::drive::MediaType::BluRay => "BluRay",
            crate::drive::MediaType::DVD => "DVD",
            _ => "Video",
        };
        format!("{}_{}", prefix, timestamp)
    };
    let dvd_dir = media_output.join(folder_name.clone());
    
    // Create completed subfolder
    let completed_dir = dvd_dir.join("completed");
    if let Err(e) = tokio::fs::create_dir_all(&completed_dir).await {
        send_log_to_web_ui(state, device, "warning", format!("⚠️  Failed to create completed folder: {}", e), Some(operation_id)).await;
    } else {
        send_log_to_web_ui(state, device, "info", format!("📁 Created completed subfolder: {}", completed_dir.display()), Some(operation_id)).await;
    }
    
    send_log_to_web_ui(state, device, "info", format!("Output: {}", dvd_dir.display()), Some(operation_id)).await;
    update_operation_progress(state, operation_id, 5.0, format!("Starting rip to: {}", dvd_dir.display())).await;
    
    let device_clone = device.to_string();
    let state_clone = state.clone();
    let operation_id_clone = operation_id.to_string();
    let state_progress = state.clone();
    let operation_id_progress = operation_id.to_string();
    
    let completed_dir_clone = completed_dir.clone();
    let metadata_for_episode = dvd_metadata.clone();
    let state_for_episode = state.clone();
    let device_for_episode = device.to_string();
    let operation_id_for_episode = operation_id.to_string();
    
    let result = crate::dvd_ripper::rip_dvd(
        device,
        &dvd_dir,
        dvd_metadata.as_ref(),
        move |progress| {
            let state = state_progress.clone();
            let operation_id = operation_id_progress.clone();
            
            tokio::spawn(async move {
                let progress_pct = progress.percentage;
                let message = format!("Track {}/{}: {} ({:.1}%)", 
                    progress.current_track, 
                    progress.total_tracks,
                    progress.track_name,
                    progress_pct);
                update_operation_progress(&state, &operation_id, progress_pct, message).await;
            });
        },
        move |log_line| {
            let state = state_clone.clone();
            let operation_id = operation_id_clone.clone();
            let device = device_clone.clone();
            
            tokio::spawn(async move {
                send_log_to_web_ui(&state, &device, "info", log_line, Some(&operation_id)).await;
            });
        },
        move |file_path: &std::path::Path, title_num: u32| {
            let completed_dir = completed_dir_clone.clone();
            let metadata = metadata_for_episode.clone();
            let state = state_for_episode.clone();
            let device = device_for_episode.clone();
            let operation_id = operation_id_for_episode.clone();
            let file_path_clone = file_path.to_path_buf();
            
            Box::pin(async move {
                process_episode_immediately(
                    &state,
                    &device,
                    &operation_id,
                    &file_path_clone,
                    &completed_dir,
                    metadata.as_ref(),
                ).await
            })
        },
    ).await;
    
    tracing::info!("Rip result received, checking: result.is_ok()={}", result.is_ok());
    match result {
        Ok(_) => {
            tracing::info!("Rip completed successfully - all episodes processed immediately");
            send_log_to_web_ui(state, device, "success", format!("✅ {} rip complete - all episodes processed", media_name), Some(operation_id)).await;
            update_operation_progress(state, operation_id, 100.0, format!("{} rip complete", media_name)).await;
            
            // Send notification
            let disc_type = match media_type {
                crate::drive::MediaType::BluRay => crate::notifications::DiscType::BluRay,
                crate::drive::MediaType::DVD => crate::notifications::DiscType::DVD,
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
            
            update_operation_progress(state, operation_id, 100.0, "Rip completed successfully".to_string()).await;
            
            // Always eject disc when done
            match crate::drive::eject_disc(device).await {
                Ok(_) => {
                    send_log_to_web_ui(state, device, "info", format!("⏏️  Ejected {}", device), Some(operation_id)).await;
                }
                Err(e) => {
                    send_log_to_web_ui(state, device, "warning", format!("⚠️  Failed to eject {}: {}", device, e), Some(operation_id)).await;
                }
            }
        }
        Err(e) => {
            send_log_to_web_ui(state, device, "error", format!("❌ {} rip failed: {}", media_name, e), Some(operation_id)).await;
            update_operation_progress(state, operation_id, 0.0, format!("Rip failed: {}", e)).await;
            
            // Send failure notification
            let disc_type = match media_type {
                crate::drive::MediaType::BluRay => crate::notifications::DiscType::BluRay,
                crate::drive::MediaType::DVD => crate::notifications::DiscType::DVD,
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
            return Err(e);
        }
    }
    
    Ok(())
}

/// Process a single episode immediately after ripping: OpenAI -> Validate -> Filebot -> Move to completed
async fn process_episode_immediately(
    state: &ApiState,
    device: &str,
    operation_id: &str,
    file_path: &std::path::Path,
    completed_dir: &std::path::Path,
    metadata: Option<&crate::dvd_metadata::DvdMetadata>,
) -> anyhow::Result<()> {
    use anyhow::Context;
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let file_name = file_path.file_name()
        .and_then(|n| n.to_str())
        .context("Invalid file path")?;
    
    send_log_to_web_ui(state, device, "info", format!("🔄 Processing episode: {}", file_name), Some(operation_id)).await;
    
    // Simple renaming: DISC_LABEL-TIMESTAMP.mkv
    let disc_label = metadata
        .map(|m| m.title.replace(' ', "_").replace("/", "_"))
        .unwrap_or_else(|| "Unknown".to_string());
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let new_name = format!("{}-{}.mkv", disc_label, timestamp);
    let final_file_path = file_path.parent().unwrap().join(&new_name);
    
    // Rename the file
    if let Err(e) = tokio::fs::rename(&file_path, &final_file_path).await {
        send_log_to_web_ui(state, device, "warning", format!("  ⚠️  Failed to rename: {}", e), Some(operation_id)).await;
        return Err(anyhow::anyhow!("Failed to rename file: {}", e));
    }
    
    send_log_to_web_ui(state, device, "success", format!("  ✅ Renamed to: {}", new_name), Some(operation_id)).await;
    
    // Move to completed folder
    send_log_to_web_ui(state, device, "info", "📦 Moving to completed folder...".to_string(), Some(operation_id)).await;
    
    // Verify file exists before trying to move it
    if !final_file_path.exists() {
        send_log_to_web_ui(state, device, "error", format!("❌ File does not exist: {}", final_file_path.display()), Some(operation_id)).await;
        return Err(anyhow::anyhow!("File does not exist: {}", final_file_path.display()));
    }
    
    let final_file_name = final_file_path.file_name()
        .and_then(|n| n.to_str())
        .context("Invalid file path")?;
    let mut dest_path = completed_dir.join(final_file_name);
    
    // Check if file already exists in completed folder
    if dest_path.exists() {
        send_log_to_web_ui(state, device, "warning", format!("  ⚠️  File {} already exists in completed, using DUPLICATE naming", final_file_name), Some(operation_id)).await;
        let mut duplicate_num = 1u32;
        while dest_path.exists() {
            let stem = std::path::Path::new(final_file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("episode");
            let ext = std::path::Path::new(final_file_name)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("mkv");
            let duplicate_name = format!("{}-DUPLICATE-{}.{}", stem, duplicate_num, ext);
            dest_path = completed_dir.join(duplicate_name);
            duplicate_num += 1;
        }
    }
    
    match tokio::fs::rename(&final_file_path, &dest_path).await {
        Ok(_) => {
            send_log_to_web_ui(state, device, "success", format!("✅ Moved to completed: {}", dest_path.file_name().unwrap().to_string_lossy()), Some(operation_id)).await;
            Ok(())
        }
        Err(e) => {
            send_log_to_web_ui(state, device, "error", format!("❌ Failed to move to completed: {} (file: {})", e, final_file_path.display()), Some(operation_id)).await;
            Err(anyhow::anyhow!("Failed to move file to completed: {} (file: {})", e, final_file_path.display()))
        }
    }
}

/// Helper to get DVD volume name (extracted from app.rs)
async fn get_dvd_volume_name(device: &str) -> anyhow::Result<String> {
    tracing::debug!("Getting volume name for device: {}", device);
    
    #[cfg(target_os = "macos")]
    {
        get_dvd_volume_name_macos(device).await
    }
    
    #[cfg(target_os = "linux")]
    {
        get_dvd_volume_name_linux(device).await
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!("Getting volume name not supported on this platform"))
    }
}

#[cfg(target_os = "macos")]
async fn get_dvd_volume_name_macos(device: &str) -> anyhow::Result<String> {
    let output = tokio::process::Command::new("diskutil")
        .arg("info")
        .arg(device)
        .output()
        .await?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    tracing::debug!("diskutil info output for volume name extraction:\n{}", stdout);
    
    let volume_name = stdout.lines()
        .find(|line| line.trim().starts_with("Volume Name:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "Not applicable (no file system)")
        .ok_or_else(|| {
            tracing::warn!("No 'Volume Name:' field found in diskutil output");
            anyhow::anyhow!("No volume name found")
        })?;
    
    tracing::debug!("Extracted volume name: {}", volume_name);
    Ok(volume_name)
}

#[cfg(target_os = "linux")]
async fn get_dvd_volume_name_linux(device: &str) -> anyhow::Result<String> {
    if let Ok(output) = tokio::process::Command::new("lsblk")
        .arg("-n")
        .arg("-o")
        .arg("MOUNTPOINT,LABEL")
        .arg(device)
        .output()
        .await
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let mount = parts[0];
                let label = parts[1];
                
                if mount != "null" && !mount.is_empty() && label != "null" && !label.is_empty() {
                    tracing::debug!("Found volume label via lsblk: {}", label);
                    return Ok(label.to_string());
                }
            }
        }
    }
    
    Err(anyhow::anyhow!("No volume name found for device {}", device))
}

/// Extract season/episode from filename like "Show.S01E02.Title.mkv"
fn extract_episode_from_filename(filename: &str) -> Option<(u32, u32)> {
    let re = regex::Regex::new(r"S(\d+)E(\d+)").ok()?;
    if let Some(caps) = re.captures(filename) {
        if let (Ok(season), Ok(episode)) = (caps[1].parse::<u32>(), caps[2].parse::<u32>()) {
            return Some((season, episode));
        }
    }
    None
}

fn create_dummy_metadata() -> crate::metadata::DiscMetadata {
    let track_count = 10;
    let tracks: Vec<crate::metadata::Track> = (1..=track_count)
        .map(|n| crate::metadata::Track {
            number: n,
            title: format!("Track {:02}", n),
            artist: None,
            duration: None,
        })
        .collect();

    crate::metadata::DiscMetadata {
        artist: "Unknown Artist".to_string(),
        album: format!("Unknown Album {}", chrono::Local::now().format("%Y-%m-%d")),
        year: Some(chrono::Local::now().format("%Y").to_string()),
        genre: None,
        tracks,
    }
}

/// Create upscaling job(s) for ripped video files
async fn create_upscaling_job_for_rip(
    state: &ApiState,
    output_path: &str,
    title: Option<&str>,
) -> anyhow::Result<()> {
    use std::path::PathBuf;
    use tokio::fs;
    
    let output_dir = PathBuf::from(output_path);
    
    // Check if directory exists and contains MKV files
    if !output_dir.exists() || !output_dir.is_dir() {
        return Ok(()); // Not a directory or doesn't exist - skip
    }
    
    // Find all MKV files in the output directory
    let mut entries = fs::read_dir(&output_dir).await?;
    let mut mkv_files = Vec::new();
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("mkv") {
            mkv_files.push(path);
        }
    }
    
    if mkv_files.is_empty() {
        tracing::debug!("No MKV files found in {} - skipping upscaling job creation", output_path);
        return Ok(()); // No video files to upscale
    }
    
    // Get show ID from title or last selected show
    let show_id = if let Some(title_str) = title {
        // Try to find show by name
        match state.db.get_shows() {
            Ok(shows) => {
                shows.iter()
                    .find(|s| s.name == title_str)
                    .and_then(|s| s.id)
            }
            Err(_) => None,
        }
    } else {
        // Use last selected show
        state.db.get_last_show_id().ok().flatten()
    };
    
    // Get Topaz profiles for the show
    let profiles = if let Some(sid) = show_id {
        match state.db.get_profiles_for_show(sid) {
            Ok(profiles) => profiles,
            Err(e) => {
                tracing::debug!("Failed to get profiles for show {}: {}", sid, e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    
    if profiles.is_empty() {
        tracing::debug!("No Topaz profiles found for show - skipping upscaling job creation");
        return Ok(()); // No profiles associated - skip upscaling
    }
    
    // Create upscaling job for each MKV file with each profile
    for mkv_file in mkv_files {
        let file_path = mkv_file.to_string_lossy().to_string();
        let file_stem = std::path::Path::new(&file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        
        for profile in &profiles {
            if let Some(profile_id) = profile.id {
                let job_id = format!("upscale_{}_{}_{}", 
                    file_stem,
                    profile_id,
                    chrono::Utc::now().timestamp());
                
                match state.db.create_upscaling_job(
                    &job_id,
                    &file_path,
                    show_id,
                    Some(profile_id),
                    0, // Default priority
                ) {
                    Ok(_) => {
                        tracing::info!("Created upscaling job {} for {} with profile {}", job_id, file_path, profile.name);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create upscaling job for {} with profile {}: {}", file_path, profile.name, e);
                    }
                }
            }
        }
    }
    
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
        operation_id: None,
    });
    
    // Check for pending upscaling jobs for files in this directory
    let directory_path = std::path::PathBuf::from(&request.directory);
    if directory_path.exists() {
        // Find all MKV files in the directory
        let mut mkv_files = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(&directory_path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("mkv") {
                    if let Some(file_str) = path.to_str() {
                        mkv_files.push(file_str.to_string());
                    }
                }
            }
        }
        
        // Check if any of these files have pending upscaling jobs
        let mut pending_jobs = Vec::new();
        for file_path in &mkv_files {
            if let Ok(jobs) = state.db.get_upscaling_jobs(None) {
                for job in jobs {
                    if job.input_file_path == *file_path 
                        && (job.status == crate::database::JobStatus::Queued 
                            || job.status == crate::database::JobStatus::Assigned
                            || job.status == crate::database::JobStatus::Processing) {
                        pending_jobs.push(job.job_id.clone());
                    }
                }
            }
        }
        
        if !pending_jobs.is_empty() {
            let _ = state.event_tx.send(ApiEvent::Log {
                level: "warning".to_string(),
                message: format!("Waiting for {} upscaling job(s) to complete before renaming...", pending_jobs.len()),
                drive: None,
                operation_id: None,
            });
            
            // Poll for job completion (with timeout)
            let start_time = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(3600); // 1 hour timeout
            
            while start_time.elapsed() < timeout {
                let mut all_complete = true;
                for job_id in &pending_jobs {
                    if let Ok(jobs) = state.db.get_upscaling_jobs(None) {
                        if let Some(job) = jobs.iter().find(|j| j.job_id == *job_id) {
                            if job.status == crate::database::JobStatus::Queued 
                                || job.status == crate::database::JobStatus::Assigned
                                || job.status == crate::database::JobStatus::Processing {
                                all_complete = false;
                                break;
                            }
                        }
                    }
                }
                
                if all_complete {
                    let _ = state.event_tx.send(ApiEvent::Log {
                        level: "info".to_string(),
                        message: "All upscaling jobs completed, proceeding with rename".to_string(),
                        drive: None,
                        operation_id: None,
                    });
                    break;
                }
                
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
            
            if start_time.elapsed() >= timeout {
                let _ = state.event_tx.send(ApiEvent::Log {
                    level: "warning".to_string(),
                    message: "Timeout waiting for upscaling jobs, proceeding with rename anyway".to_string(),
                    drive: None,
                    operation_id: None,
                });
            }
        }
    }
    
    // Call existing rename functionality
    let directory = if request.directory.is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(&request.directory))
    };
    
    match crate::rename::run_rename(
        directory,
        request.title.clone(),
        request.skip_speech,
        request.skip_filebot,
    ).await {
        Ok(_) => {
            let _ = state.event_tx.send(ApiEvent::Log {
                level: "success".to_string(),
                message: "Rename operation completed successfully".to_string(),
                drive: None,
                operation_id: None,
            });
        }
        Err(e) => {
            let _ = state.event_tx.send(ApiEvent::Log {
                level: "error".to_string(),
                message: format!("Rename operation failed: {}", e),
                drive: None,
                operation_id: None,
            });
            return Err(e);
        }
    }
    
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
#[allow(dead_code)]
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
    
    // Create operation for this queued rip
    let operation_id = create_operation(
        &state,
        OperationType::Rip,
        Some(drive_id.clone()),
        next_entry.title.clone(),
        format!("Starting queued rip on drive {}", drive_id),
    ).await;
    
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
    let operation_id_clone = operation_id.clone();
    let output_path_clone = request.output_path.clone();
    let title_clone = request.title.clone();
    
    // Spawn rip operation
    tokio::spawn(async move {
        let result = run_rip_operation(state_clone.clone(), request, drive_clone.clone(), operation_id_clone.clone()).await;
        
        // Complete or fail the operation
        if let Err(ref e) = result {
            fail_operation(&state_clone, &operation_id_clone, format!("{}", e)).await;
        } else {
            // Try to create upscaling job if this was a video rip
            if let Some(ref output_path) = output_path_clone {
                if let Err(e) = create_upscaling_job_for_rip(&state_clone, output_path, title_clone.as_deref()).await {
                    tracing::warn!("Failed to create upscaling job after queued rip: {}", e);
                }
            }
            
            complete_operation(&state_clone, &operation_id_clone, Some("Queued rip completed successfully".to_string())).await;
        }
        
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

/// Get monitor operations (active operations)
/// Get monitor operations (including upscaling jobs)
async fn get_monitor_operations(State(state): State<ApiState>) -> Result<Json<Vec<Operation>>, ErrorResponse> {
    let operations = state.operations.read().await;
    let mut all_operations: Vec<Operation> = operations.values().cloned().collect();
    
    // Also include upscaling jobs as operations
    match state.db.get_upscaling_jobs(None) {
        Ok(jobs) => {
            for job in jobs {
                // Only show active or recent jobs
                if job.status == crate::database::JobStatus::Processing 
                   || job.status == crate::database::JobStatus::Assigned 
                   || job.status == crate::database::JobStatus::Queued
                   || job.status == crate::database::JobStatus::Completed 
                   || job.status == crate::database::JobStatus::Failed {
                    // Convert job to operation format
                    let operation_id = format!("upscale_{}", job.job_id);
                    
                    // Check if we already have this operation
                    if all_operations.iter().any(|op| op.operation_id == operation_id) {
                        continue;
                    }
                    
                    let status = match job.status {
                        crate::database::JobStatus::Queued => OperationStatus::Queued,
                        crate::database::JobStatus::Assigned => OperationStatus::Queued,
                        crate::database::JobStatus::Processing => OperationStatus::Running,
                        crate::database::JobStatus::Completed => OperationStatus::Completed,
                        crate::database::JobStatus::Failed => OperationStatus::Failed,
                        crate::database::JobStatus::Cancelled => OperationStatus::Failed,
                    };
                    
                    let input_file = std::path::Path::new(&job.input_file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    
                    let message = if let Some(profile_id) = job.topaz_profile_id {
                        format!("Upscaling {} with profile {}", input_file, profile_id)
                    } else {
                        format!("Upscaling {}", input_file)
                    };
                    
                    // Get started_at - use created_at if started_at is None
                    let started_at = job.started_at
                        .unwrap_or(job.created_at);
                    
                    let operation = Operation {
                        operation_id: operation_id.clone(),
                        operation_type: OperationType::Upscale,
                        status,
                        title: Some(format!("Upscale: {}", input_file)),
                        message,
                        progress: job.progress,
                        drive: None,
                        started_at,
                        completed_at: job.completed_at,
                        error: job.error_message,
                    };
                    
                    all_operations.push(operation);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to fetch upscaling jobs for monitor: {}", e);
        }
    }
    
    Ok(Json(all_operations))
}

/// Get operation history (completed/failed operations)
async fn get_operation_history(
    State(state): State<ApiState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<Operation>>, ErrorResponse> {
    let limit = params.get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(50);
    let status_filter = params.get("status").map(|s| s.as_str());
    
    match state.db.get_operation_history(Some(limit), status_filter) {
        Ok(history) => Ok(Json(history)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get operation history: {}", e),
        }),
    }
}

/// Get monitor drives information
async fn get_monitor_drives(State(_state): State<ApiState>) -> Result<Json<Vec<crate::drive::DriveInfo>>, ErrorResponse> {
    match crate::drive::detect_drives().await {
        Ok(drives) => Ok(Json(drives)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to detect drives: {}", e),
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

// Agent API endpoints

/// Agent registration request
#[derive(Debug, Deserialize)]
struct AgentRegistrationRequest {
    agent_id: String,
    name: String,
    platform: String,
    capabilities: Option<String>,
    topaz_version: Option<String>,
    api_key: Option<String>,
    os_version: Option<String>,
    os_arch: Option<String>,
}

/// Register a new agent or update existing agent
async fn register_agent(
    State(state): State<ApiState>,
    Json(request): Json<AgentRegistrationRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    // Extract IP address from request (if available)
    let ip_address: Option<&str> = None; // TODO: Extract from request headers
    
    match state.db.register_agent(
        &request.agent_id,
        &request.name,
        &request.platform,
        #[allow(clippy::needless_option_as_deref)]
        ip_address.as_deref(),
        request.capabilities.as_deref(),
        request.topaz_version.as_deref(),
        request.api_key.as_deref(),
        request.os_version.as_deref(),
        request.os_arch.as_deref(),
    ) {
        Ok(_) => {
            info!("Agent registered: {} ({})", request.name, request.agent_id);
            
            // Broadcast agent status change via WebSocket
            let _ = state.event_tx.send(ApiEvent::AgentStatusChanged {
                agent_id: request.agent_id.clone(),
                status: "online".to_string(),
                last_seen: chrono::Utc::now().to_rfc3339(),
                operation_id: None,
            });
            
            Ok(Json(serde_json::json!({
                "success": true,
                "agent_id": request.agent_id,
                "message": "Agent registered successfully"
            })))
        }
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to register agent: {}", e),
        }),
    }
}

/// Update agent heartbeat
#[derive(Debug, Deserialize)]
struct AgentHeartbeatRequest {
    status: Option<String>,
}

async fn agent_heartbeat(
    State(state): State<ApiState>,
    axum::extract::Path(agent_id): axum::extract::Path<String>,
    Json(request): Json<AgentHeartbeatRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.update_agent_heartbeat(&agent_id, request.status.as_deref()) {
        Ok(_) => {
            // Broadcast agent heartbeat update via WebSocket
            let _ = state.event_tx.send(ApiEvent::AgentStatusChanged {
                agent_id: agent_id.clone(),
                status: request.status.as_deref().unwrap_or("online").to_string(),
                last_seen: chrono::Utc::now().to_rfc3339(),
                operation_id: None,
            });
            
            Ok(Json(serde_json::json!({
                "success": true,
                "agent_id": agent_id
            })))
        }
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to update heartbeat: {}", e),
        }),
    }
}

/// Get list of all agents
async fn get_agents(
    State(state): State<ApiState>,
) -> Result<Json<Vec<AgentInfo>>, ErrorResponse> {
    match state.db.get_agents() {
        Ok(agents) => Ok(Json(agents)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get agents: {}", e),
        }),
    }
}

/// Get pending instructions for an agent (including completed ones with output for display)
async fn get_agent_instructions(
    State(state): State<ApiState>,
    axum::extract::Path(agent_id): axum::extract::Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, ErrorResponse> {
    tracing::debug!("[GET_INSTRUCTIONS] Agent {} requesting instructions", agent_id);
    
    // Get pending instructions
    match state.db.get_pending_instructions(Some(&agent_id)) {
        Ok(mut instructions) => {
            tracing::debug!("[GET_INSTRUCTIONS] Found {} pending/assigned instructions for agent {}", instructions.len(), agent_id);
            
            // Auto-assign the first pending instruction to this agent if not already assigned
            if let Some(instruction) = instructions.first_mut() {
                if let Some(id_val) = instruction.get("id").and_then(|v| v.as_i64()) {
                    if instruction.get("assigned_to_agent_id").is_none() {
                        tracing::info!("[GET_INSTRUCTIONS] Auto-assigning instruction_id={} to agent_id={}", id_val, agent_id);
                        // Assign instruction to this agent
                        if let Err(e) = state.db.assign_instruction_to_agent(id_val, &agent_id) {
                            tracing::error!("[GET_INSTRUCTIONS] Failed to assign instruction_id={} to agent_id={}: {}", id_val, agent_id, e);
                            return Err(ErrorResponse {
                                error: format!("Failed to assign instruction: {}", e),
                            });
                        }
                        instruction["assigned_to_agent_id"] = serde_json::Value::String(agent_id.clone());
                        instruction["status"] = serde_json::Value::String("assigned".to_string());
                    } else {
                        let assigned_to = instruction.get("assigned_to_agent_id").and_then(|v| v.as_str()).unwrap_or("none");
                        tracing::debug!("[GET_INSTRUCTIONS] Instruction_id={} already assigned to {}", id_val, assigned_to);
                    }
                }
            }
            
            // Log all instructions being returned
            for inst in &instructions {
                if let (Some(id), Some(typ), Some(status)) = (
                    inst.get("id").and_then(|v| v.as_i64()),
                    inst.get("instruction_type").and_then(|v| v.as_str()),
                    inst.get("status").and_then(|v| v.as_str())
                ) {
                    tracing::info!("[GET_INSTRUCTIONS] Returning instruction: id={}, type={}, status={}", id, typ, status);
                }
            }
            
            // Also get recent completed instructions for this agent (last 5) to show output
            let completed = match state.db.get_recent_completed_instructions(&agent_id, 5) {
                Ok(insts) => {
                    tracing::debug!("[GET_INSTRUCTIONS] Found {} recent completed instructions", insts.len());
                    insts
                }
                Err(e) => {
                    tracing::warn!("[GET_INSTRUCTIONS] Failed to get recent completed instructions for agent {}: {}", agent_id, e);
                    vec![] // If query fails, just return pending instructions
                }
            };
            
            // Combine pending and recent completed
            instructions.extend(completed);
            tracing::debug!("[GET_INSTRUCTIONS] Returning total of {} instructions to agent {}", instructions.len(), agent_id);
            Ok(Json(instructions))
        }
        Err(e) => {
            tracing::error!("[GET_INSTRUCTIONS] Failed to get instructions for agent {}: {}", agent_id, e);
            Err(ErrorResponse {
                error: format!("Failed to get instructions: {}", e),
            })
        }
    }
}

/// Get agent output location
async fn get_agent_output_location(
    State(state): State<ApiState>,
    axum::extract::Path(agent_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.get_agent_by_id(&agent_id) {
        Ok(Some(agent)) => Ok(Json(serde_json::json!({
            "output_location": agent.output_location
        }))),
        Ok(None) => Err(ErrorResponse {
            error: format!("Agent not found: {}", agent_id),
        }),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get agent: {}", e),
        }),
    }
}

/// Update agent output location
#[derive(Debug, Deserialize)]
struct UpdateAgentOutputLocationRequest {
    output_location: String,
}

async fn update_agent_output_location(
    State(state): State<ApiState>,
    axum::extract::Path(agent_id): axum::extract::Path<String>,
    Json(request): Json<UpdateAgentOutputLocationRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.update_agent_output_location(&agent_id, &request.output_location) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "agent_id": agent_id,
            "output_location": request.output_location
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to update output location: {}", e),
        }),
    }
}

/// Disconnect an agent (mark as offline)
async fn disconnect_agent(
    State(state): State<ApiState>,
    axum::extract::Path(agent_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.update_agent_heartbeat(&agent_id, Some("offline")) {
        Ok(_) => {
            // Broadcast agent status change via WebSocket
            let _ = state.event_tx.send(ApiEvent::AgentStatusChanged {
                agent_id: agent_id.clone(),
                status: "offline".to_string(),
                last_seen: chrono::Utc::now().to_rfc3339(),
                operation_id: None,
            });
            
            Ok(Json(serde_json::json!({
                "success": true,
                "agent_id": agent_id,
                "message": "Agent disconnected"
            })))
        }
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to disconnect agent: {}", e),
        }),
    }
}

/// Delete an agent
async fn delete_agent(
    State(state): State<ApiState>,
    axum::extract::Path(agent_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.delete_agent(&agent_id) {
        Ok(_) => {
            // Broadcast agent status change via WebSocket
            let _ = state.event_tx.send(ApiEvent::AgentStatusChanged {
                agent_id: agent_id.clone(),
                status: "deleted".to_string(),
                last_seen: chrono::Utc::now().to_rfc3339(),
                operation_id: None,
            });
            
            Ok(Json(serde_json::json!({
                "success": true,
                "agent_id": agent_id,
                "message": "Agent deleted"
            })))
        }
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to delete agent: {}", e),
        }),
    }
}

/// Test command request
#[derive(Debug, Deserialize)]
struct TestCommandRequest {
    command: String,
}

/// Test agent command (for validation)
async fn test_agent_command(
    State(state): State<ApiState>,
    axum::extract::Path(agent_id): axum::extract::Path<String>,
    Json(request): Json<TestCommandRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    tracing::info!("[TEST_COMMAND] Received test command request for agent_id={}, command={}", agent_id, request.command);
    
    // Create a test instruction for the agent
    let payload = serde_json::json!({
        "command": request.command,
        "test": true,
    });
    
    tracing::debug!("[TEST_COMMAND] Creating instruction with payload: {:?}", payload);
    
    match state.db.create_instruction("test_command", &payload) {
        Ok(instruction_id) => {
            tracing::info!("[TEST_COMMAND] Created instruction_id={} for agent_id={}", instruction_id, agent_id);
            
            // Assign to the specific agent
            match state.db.assign_instruction_to_agent(instruction_id, &agent_id) {
                Ok(_) => {
                    tracing::info!("[TEST_COMMAND] Successfully assigned instruction_id={} to agent_id={}", instruction_id, agent_id);
                    
                    // Verify the instruction was assigned correctly
                    if let Ok(Some(instruction)) = state.db.get_instruction(instruction_id) {
                        tracing::info!("[TEST_COMMAND] Verified instruction: id={}, status={}, assigned_to={}", 
                            instruction_id,
                            instruction.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"),
                            instruction.get("assigned_to_agent_id").and_then(|v| v.as_str()).unwrap_or("none")
                        );
                    }
                    
                    Ok(Json(serde_json::json!({
                        "success": true,
                        "instruction_id": instruction_id,
                        "agent_id": agent_id,
                        "message": "Test command sent to agent"
                    })))
                }
                Err(e) => {
                    tracing::error!("[TEST_COMMAND] Failed to assign instruction_id={} to agent_id={}: {}", instruction_id, agent_id, e);
                    Err(ErrorResponse {
                        error: format!("Failed to assign test command: {}", e),
                    })
                }
            }
        }
        Err(e) => {
            tracing::error!("[TEST_COMMAND] Failed to create instruction: {}", e);
            Err(ErrorResponse {
                error: format!("Failed to create test command: {}", e),
            })
        }
    }
}

/// Create a new instruction
#[derive(Debug, Deserialize)]
struct CreateInstructionRequest {
    instruction_type: String,
    payload: serde_json::Value,
}

async fn create_instruction(
    State(state): State<ApiState>,
    Json(request): Json<CreateInstructionRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.create_instruction(&request.instruction_type, &request.payload) {
        Ok(id) => Ok(Json(serde_json::json!({
            "success": true,
            "instruction_id": id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to create instruction: {}", e),
        }),
    }
}

/// Assign an instruction to an agent
#[derive(Debug, Deserialize)]
struct AssignInstructionRequest {
    agent_id: String,
}

async fn assign_instruction(
    State(state): State<ApiState>,
    axum::extract::Path(instruction_id): axum::extract::Path<i64>,
    Json(request): Json<AssignInstructionRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.assign_instruction_to_agent(instruction_id, &request.agent_id) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "instruction_id": instruction_id,
            "agent_id": request.agent_id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to assign instruction: {}", e),
        }),
    }
}

/// Mark instruction as started
async fn start_instruction(
    State(state): State<ApiState>,
    axum::extract::Path(instruction_id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    tracing::info!("[START_INSTRUCTION] Agent requesting to start instruction_id={}", instruction_id);
    
    // Get current instruction status before updating
    if let Ok(Some(inst)) = state.db.get_instruction(instruction_id) {
        let old_status = inst.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
        tracing::info!("[START_INSTRUCTION] Current status: {}", old_status);
    }
    
    match state.db.start_instruction(instruction_id) {
        Ok(_) => {
            tracing::info!("[START_INSTRUCTION] Successfully marked instruction_id={} as started (status=processing)", instruction_id);
            
            // Verify the status was updated
            if let Ok(Some(inst)) = state.db.get_instruction(instruction_id) {
                let new_status = inst.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                tracing::info!("[START_INSTRUCTION] Verified new status: {}", new_status);
            }
            
            Ok(Json(serde_json::json!({
                "success": true,
                "instruction_id": instruction_id
            })))
        }
        Err(e) => {
            tracing::error!("[START_INSTRUCTION] Failed to start instruction_id={}: {}", instruction_id, e);
            Err(ErrorResponse {
                error: format!("Failed to start instruction: {}", e),
            })
        }
    }
}

/// Complete instruction request
#[derive(Debug, Deserialize)]
struct CompleteInstructionRequest {
    output: Option<String>,
}

/// Mark instruction as completed
async fn complete_instruction(
    State(state): State<ApiState>,
    axum::extract::Path(instruction_id): axum::extract::Path<i64>,
    Json(request): Json<CompleteInstructionRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let output_len = request.output.as_ref().map(|s| s.len()).unwrap_or(0);
    tracing::info!("[COMPLETE_INSTRUCTION] Agent completing instruction_id={} with output length={}", instruction_id, output_len);
    
    if let Some(ref output) = request.output {
        tracing::debug!("[COMPLETE_INSTRUCTION] Output preview (first 200 chars): {}", 
            if output.len() > 200 { &output[..200] } else { output });
    }
    
    match state.db.complete_instruction(instruction_id, request.output.as_deref()) {
        Ok(_) => {
            tracing::info!("[COMPLETE_INSTRUCTION] Successfully marked instruction_id={} as completed", instruction_id);
            
            // Verify the status was updated
            if let Ok(Some(inst)) = state.db.get_instruction(instruction_id) {
                let new_status = inst.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                let has_output = inst.get("output").is_some();
                tracing::info!("[COMPLETE_INSTRUCTION] Verified: status={}, has_output={}", new_status, has_output);
            }
            
            Ok(Json(serde_json::json!({
                "success": true,
                "instruction_id": instruction_id
            })))
        }
        Err(e) => {
            tracing::error!("[COMPLETE_INSTRUCTION] Failed to complete instruction_id={}: {}", instruction_id, e);
            Err(ErrorResponse {
                error: format!("Failed to complete instruction: {}", e),
            })
        }
    }
}

/// Get instruction by ID
async fn get_instruction(
    State(state): State<ApiState>,
    axum::extract::Path(instruction_id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.get_instruction(instruction_id) {
        Ok(Some(instruction)) => Ok(Json(instruction)),
        Ok(None) => Err(ErrorResponse {
            error: "Instruction not found".to_string(),
        }),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get instruction: {}", e),
        }),
    }
}

/// Mark instruction as failed
#[derive(Debug, Deserialize)]
struct FailInstructionRequest {
    error_message: String,
}

async fn fail_instruction(
    State(state): State<ApiState>,
    axum::extract::Path(instruction_id): axum::extract::Path<i64>,
    Json(request): Json<FailInstructionRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.fail_instruction(instruction_id, &request.error_message) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "instruction_id": instruction_id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to fail instruction: {}", e),
        }),
    }
}

// Topaz Profile API endpoints

/// Get all Topaz profiles
async fn get_topaz_profiles(
    State(state): State<ApiState>,
) -> Result<Json<Vec<TopazProfile>>, ErrorResponse> {
    match state.db.get_topaz_profiles() {
        Ok(profiles) => Ok(Json(profiles)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get Topaz profiles: {}", e),
        }),
    }
}

/// Create a new Topaz profile
#[derive(Debug, Deserialize)]
struct CreateTopazProfileRequest {
    name: String,
    command: String, // Command to execute for this profile
}

async fn create_topaz_profile(
    State(state): State<ApiState>,
    Json(request): Json<CreateTopazProfileRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.create_topaz_profile(
        &request.name,
        &request.command,
    ) {
        Ok(id) => Ok(Json(serde_json::json!({
            "success": true,
            "profile_id": id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to create Topaz profile: {}", e),
        }),
    }
}

/// Get a Topaz profile by ID
async fn get_topaz_profile(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<TopazProfile>, ErrorResponse> {
    match state.db.get_topaz_profile(id) {
        Ok(Some(profile)) => Ok(Json(profile)),
        Ok(None) => Err(ErrorResponse {
            error: "Topaz profile not found".to_string(),
        }),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get Topaz profile: {}", e),
        }),
    }
}

/// Update a Topaz profile
#[derive(Debug, Deserialize)]
struct UpdateTopazProfileRequest {
    name: Option<String>,
    command: Option<String>, // Command to execute for this profile
}

async fn update_topaz_profile(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(request): Json<UpdateTopazProfileRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.update_topaz_profile(
        id,
        request.name.as_deref(),
        request.command.as_deref(),
    ) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "profile_id": id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to update Topaz profile: {}", e),
        }),
    }
}

/// Delete a Topaz profile
async fn delete_topaz_profile(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.delete_topaz_profile(id) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "profile_id": id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to delete Topaz profile: {}", e),
        }),
    }
}

/// Associate a Topaz profile with a show
async fn associate_profile_with_show(
    State(state): State<ApiState>,
    axum::extract::Path((profile_id, show_id)): axum::extract::Path<(i64, i64)>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.associate_profile_with_show(show_id, profile_id) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "profile_id": profile_id,
            "show_id": show_id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to associate profile with show: {}", e),
        }),
    }
}

/// Remove association between a Topaz profile and a show
async fn remove_profile_from_show(
    State(state): State<ApiState>,
    axum::extract::Path((profile_id, show_id)): axum::extract::Path<(i64, i64)>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.remove_profile_from_show(show_id, profile_id) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "profile_id": profile_id,
            "show_id": show_id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to remove profile from show: {}", e),
        }),
    }
}

/// Get Topaz profiles associated with a show
async fn get_profiles_for_show(
    State(state): State<ApiState>,
    axum::extract::Path(show_id): axum::extract::Path<i64>,
) -> Result<Json<Vec<TopazProfile>>, ErrorResponse> {
    match state.db.get_profiles_for_show(show_id) {
        Ok(profiles) => Ok(Json(profiles)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get profiles for show: {}", e),
        }),
    }
}

// Upscaling Job API endpoints

/// Get all upscaling jobs
async fn get_upscaling_jobs(
    State(state): State<ApiState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<UpscalingJob>>, ErrorResponse> {
    let status_filter = params.get("status")
        .map(|s| JobStatus::from_string(s));
    
    match state.db.get_upscaling_jobs(status_filter) {
        Ok(jobs) => Ok(Json(jobs)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get upscaling jobs: {}", e),
        }),
    }
}

/// Create a new upscaling job
#[derive(Debug, Deserialize)]
struct CreateUpscalingJobRequest {
    job_id: String,
    input_file_path: String,
    show_id: Option<i64>,
    topaz_profile_id: Option<i64>,
    priority: Option<i32>,
}

async fn create_upscaling_job(
    State(state): State<ApiState>,
    Json(request): Json<CreateUpscalingJobRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let priority = request.priority.unwrap_or(0);
    
    match state.db.create_upscaling_job(
        &request.job_id,
        &request.input_file_path,
        request.show_id,
        request.topaz_profile_id,
        priority,
    ) {
        Ok(id) => Ok(Json(serde_json::json!({
            "success": true,
            "job_id": request.job_id,
            "id": id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to create upscaling job: {}", e),
        }),
    }
}

/// Get next available upscaling job for assignment
async fn get_next_upscaling_job(
    State(state): State<ApiState>,
) -> Result<Json<Option<UpscalingJob>>, ErrorResponse> {
    match state.db.get_next_upscaling_job() {
        Ok(job) => Ok(Json(job)),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to get next upscaling job: {}", e),
        }),
    }
}

/// Assign an upscaling job to an agent
#[derive(Debug, Deserialize)]
struct AssignUpscalingJobRequest {
    agent_id: String,
    instruction_id: Option<i64>,
}

async fn assign_upscaling_job(
    State(state): State<ApiState>,
    axum::extract::Path(job_id): axum::extract::Path<String>,
    Json(request): Json<AssignUpscalingJobRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.assign_upscaling_job(&job_id, &request.agent_id, request.instruction_id) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "job_id": job_id,
            "agent_id": request.agent_id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to assign upscaling job: {}", e),
        }),
    }
}

/// Update upscaling job status
#[derive(Debug, Deserialize)]
struct UpdateUpscalingJobStatusRequest {
    status: String,
    progress: Option<f32>,
    error_message: Option<String>,
}

async fn update_upscaling_job_status(
    State(state): State<ApiState>,
    axum::extract::Path(job_id): axum::extract::Path<String>,
    Json(request): Json<UpdateUpscalingJobStatusRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let status = JobStatus::from_string(&request.status);
    
    match state.db.update_upscaling_job_status(
        &job_id,
        status,
        request.progress,
        request.error_message.as_deref(),
    ) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "job_id": job_id
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to update upscaling job status: {}", e),
        }),
    }
}

/// Update upscaling job output path
#[derive(Debug, Deserialize)]
struct UpdateUpscalingJobOutputRequest {
    output_file_path: String,
}

async fn update_upscaling_job_output(
    State(state): State<ApiState>,
    axum::extract::Path(job_id): axum::extract::Path<String>,
    Json(request): Json<UpdateUpscalingJobOutputRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    match state.db.update_upscaling_job_output(&job_id, &request.output_file_path) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "job_id": job_id,
            "output_file_path": request.output_file_path
        }))),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to update upscaling job output: {}", e),
        }),
    }
}

// File Transfer API endpoints

/// Get agent file storage directory
fn get_agent_storage_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".config").join("ripley").join("agent_files")
    } else {
        PathBuf::from("agent_files")
    }
}

/// Upload file from agent (multipart form data)
async fn upload_file(
    State(state): State<ApiState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    use tokio::io::AsyncWriteExt;
    
    let storage_dir = get_agent_storage_dir();
    tokio::fs::create_dir_all(&storage_dir).await
        .map_err(|e| ErrorResponse {
            error: format!("Failed to create storage directory: {}", e),
        })?;
    
    let mut agent_id: Option<String> = None;
    let mut job_id: Option<String> = None;
    let mut file_path: Option<PathBuf> = None;
    
    while let Some(field) = multipart.next_field().await
        .map_err(|e| ErrorResponse {
            error: format!("Failed to read multipart field: {}", e),
        })? {
        let field_name = field.name();
        
        match field_name {
            Some("agent_id") => {
                let data = field.text().await
                    .map_err(|e| ErrorResponse {
                        error: format!("Failed to read agent_id: {}", e),
                    })?;
                agent_id = Some(data);
            }
            Some("job_id") => {
                let data = field.text().await
                    .map_err(|e| ErrorResponse {
                        error: format!("Failed to read job_id: {}", e),
                    })?;
                job_id = Some(data);
            }
            Some("file") => {
                let filename = field.file_name()
                    .ok_or_else(|| ErrorResponse {
                        error: "Missing filename in file field".to_string(),
                    })?
                    .to_string();
                
                let dest_path = storage_dir.join(&filename);
                
                // Ensure parent directory exists
                if let Some(parent) = dest_path.parent() {
                    tokio::fs::create_dir_all(parent).await
                        .map_err(|e| ErrorResponse {
                            error: format!("Failed to create directory: {}", e),
                        })?;
                }
                
                // Save file - write directly in chunks
                let mut file = tokio::fs::File::create(&dest_path).await
                    .map_err(|e| ErrorResponse {
                        error: format!("Failed to create file: {}", e),
                    })?;
                
                // Read field bytes directly
                let bytes = field.bytes().await
                    .map_err(|e| ErrorResponse {
                        error: format!("Failed to read file data: {}", e),
                    })?;
                
                file.write_all(&bytes).await
                    .map_err(|e| ErrorResponse {
                        error: format!("Failed to write file: {}", e),
                    })?;
                
                file_path = Some(dest_path);
            }
            _ => {}
        }
    }
    
    let file_path_str = file_path.ok_or_else(|| ErrorResponse {
        error: "No file provided".to_string(),
    })?;
    
    // Update upscaling job with output path if job_id provided
    if let Some(ref jid) = job_id {
        if let Err(e) = state.db.update_upscaling_job_output(jid, file_path_str.to_string_lossy().as_ref()) {
            tracing::warn!("Failed to update upscaling job output: {}", e);
        }
    }
    
    Ok(Json(serde_json::json!({
        "success": true,
        "file_path": file_path_str.to_string_lossy(),
        "agent_id": agent_id,
        "job_id": job_id,
    })))
}

/// Download file for agent processing
async fn download_file(
    State(_state): State<ApiState>,
    axum::extract::Path(file_path): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ErrorResponse> {
    use axum::body::Body;
    use axum::http::{header, StatusCode};
    use tokio::fs;
    
    // Decode URL-encoded file path
    let decoded_path = urlencoding::decode(&file_path)
        .map_err(|_| ErrorResponse {
            error: "Invalid file path encoding".to_string(),
        })?;
    
    let path = PathBuf::from(decoded_path.as_ref());
    
    // Security: Ensure path is within allowed directories
    if !path.exists() {
        return Err(ErrorResponse {
            error: "File not found".to_string(),
        });
    }
    
    // Read file
    let file_contents = fs::read(&path).await
        .map_err(|e| ErrorResponse {
            error: format!("Failed to read file: {}", e),
        })?;
    
    // Determine MIME type
    let mime_type = mime_guess::from_path(&path)
        .first_or_octet_stream();
    
    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type.as_ref())
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
        .body(Body::from(file_contents))
        .map_err(|e| ErrorResponse {
            error: format!("Failed to build response: {}", e),
        })?;
    
    Ok(response)
}

/// Retry a failed upscaling job
async fn retry_upscaling_job(
    State(state): State<ApiState>,
    axum::extract::Path(job_id): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let max_retries = params.get("max_retries")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(3);
    
    match state.db.retry_upscaling_job(&job_id, max_retries) {
        Ok(true) => {
            info!("Retrying upscaling job: {}", job_id);
            Ok(Json(serde_json::json!({
                "success": true,
                "job_id": job_id,
                "message": "Job queued for retry"
            })))
        }
        Ok(false) => Err(ErrorResponse {
            error: format!("Job {} has exceeded maximum retry count ({})", job_id, max_retries),
        }),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to retry job: {}", e),
        }),
    }
}

/// Cleanup old upscaling jobs
#[derive(Debug, Deserialize)]
struct CleanupJobsRequest {
    days_threshold: Option<i64>,
    keep_recent: Option<i64>,
}

async fn cleanup_old_upscaling_jobs(
    State(state): State<ApiState>,
    Json(request): Json<CleanupJobsRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let days_threshold = request.days_threshold.unwrap_or(30); // Default: 30 days
    let keep_recent = request.keep_recent; // Optional: keep N most recent jobs
    
    match state.db.cleanup_old_upscaling_jobs(days_threshold, keep_recent) {
        Ok(deleted_count) => {
            info!("Cleaned up {} old upscaling jobs (threshold: {} days)", deleted_count, days_threshold);
            Ok(Json(serde_json::json!({
                "success": true,
                "deleted_count": deleted_count,
                "days_threshold": days_threshold,
                "keep_recent": keep_recent
            })))
        }
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to cleanup old jobs: {}", e),
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
            operation_id: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Log"));
        assert!(json.contains("test"));
    }
}
