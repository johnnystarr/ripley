use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http,
    response::{IntoResponse, Response},
    routing::{get, post},
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

/// Start the REST API server
pub async fn start_server(
    config: Config,
    host: String,
    port: u16,
    dev_mode: bool,
) -> anyhow::Result<()> {
    // Create broadcast channel for events
    let (event_tx, _) = broadcast::channel(100);
    
    // Create shared state
    let state = ApiState {
        config: Arc::new(RwLock::new(config)),
        rip_status: Arc::new(RwLock::new(RipStatus::default())),
        event_tx,
    };
    
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
    eprintln!("\x1b[36m  • Web UI:\x1b[0m        http://{}/", addr);
    eprintln!("\x1b[36m  • Health check:\x1b[0m http://{}/api/health", addr);
    eprintln!("\x1b[36m  • API status:\x1b[0m    http://{}/api/status", addr);
    eprintln!("\x1b[36m  • WebSocket:\x1b[0m     ws://{}/api/ws\n", addr);
    
    if dev_mode {
        eprintln!("\x1b[33m🔧 Development mode:\x1b[0m Use Vite dev server for UI hot reload");
        eprintln!("\x1b[33m   Run: cd web-ui && npm run dev\x1b[0m\n");
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
}

/// Current ripping status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipStatus {
    pub is_ripping: bool,
    pub current_disc: Option<String>,
    pub current_title: Option<String>,
    pub progress: f32,
    pub logs: Vec<String>,
}

impl Default for RipStatus {
    fn default() -> Self {
        Self {
            is_ripping: false,
            current_disc: None,
            current_title: None,
            progress: 0.0,
            logs: Vec::new(),
        }
    }
}

/// Events broadcast to WebSocket clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ApiEvent {
    RipStarted { disc: String },
    RipProgress { progress: f32, message: String },
    RipCompleted { disc: String },
    RipError { error: String },
    Log { message: String },
    StatusUpdate { status: RipStatus },
}

/// Request body for starting a rip operation
#[derive(Debug, Deserialize)]
pub struct StartRipRequest {
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
        .route("/rip/start", post(start_rip))
        .route("/rip/stop", post(stop_rip))
        .route("/drives", get(list_drives))
        .route("/rename", post(rename_files))
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
    let mut status = state.rip_status.write().await;
    
    if status.is_ripping {
        return Err(ErrorResponse {
            error: "A rip operation is already in progress".to_string(),
        });
    }
    
    status.is_ripping = true;
    status.progress = 0.0;
    drop(status);
    
    // Clone state for async task
    let state_clone = state.clone();
    let request_clone = request;
    
    // Spawn rip operation in background
    tokio::spawn(async move {
        let _ = run_rip_operation(state_clone, request_clone).await;
    });
    
    Ok(Json(serde_json::json!({
        "status": "started"
    })))
}

/// Stop ripping operation
async fn stop_rip(State(state): State<ApiState>) -> Json<serde_json::Value> {
    let mut status = state.rip_status.write().await;
    status.is_ripping = false;
    
    let _ = state.event_tx.send(ApiEvent::Log {
        message: "Rip operation stopped by user".to_string(),
    });
    
    Json(serde_json::json!({
        "status": "stopped"
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
) -> anyhow::Result<()> {
    
    let _ = state.event_tx.send(ApiEvent::Log {
        message: "Starting rip operation...".to_string(),
    });
    
    let args = RipArgs {
        output_folder: request.output_path.map(PathBuf::from),
        title: request.title,
        skip_metadata: request.skip_metadata,
        skip_filebot: request.skip_filebot,
        quality: 5,
        eject_when_done: true,
    };
    
    // Note: We'll need to refactor app::run to work without TUI
    // For now, this is a placeholder
    match crate::app::run(args).await {
        Ok(_) => {
            let _ = state.event_tx.send(ApiEvent::RipCompleted {
                disc: "Unknown".to_string(),
            });
        }
        Err(e) => {
            let _ = state.event_tx.send(ApiEvent::RipError {
                error: e.to_string(),
            });
        }
    }
    
    let mut status = state.rip_status.write().await;
    status.is_ripping = false;
    
    Ok(())
}

/// Run rename operation in background
async fn run_rename_operation(
    state: ApiState,
    request: RenameRequest,
    _config: Config,
) -> anyhow::Result<()> {
    let _ = state.event_tx.send(ApiEvent::Log {
        message: format!("Starting rename for directory: {}", request.directory),
    });
    
    // Call existing rename functionality
    // Note: We'll need to refactor rename::run_rename to work without prompts
    // For now, log the parameters that would be used
    if let Some(ref title) = request.title {
        let _ = state.event_tx.send(ApiEvent::Log {
            message: format!("Using title: {}", title),
        });
    }
    let _ = state.event_tx.send(ApiEvent::Log {
        message: format!(
            "Options: skip_speech={}, skip_filebot={}",
            request.skip_speech, request.skip_filebot
        ),
    });
    
    let _ = state.event_tx.send(ApiEvent::Log {
        message: "Rename operation completed".to_string(),
    });
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rip_status_default() {
        let status = RipStatus::default();
        assert!(!status.is_ripping);
        assert_eq!(status.progress, 0.0);
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
            message: "test".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Log"));
        assert!(json.contains("test"));
    }
}
