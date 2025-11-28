use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::tui::AppState;
use crate::api::ApiState;

/// Rsync a directory to /Volumes/video/RawRips
/// Logs progress to the rsync log window in the TUI
pub async fn rsync_to_rawrips(
    source_dir: &Path,
    tui_state: Arc<Mutex<AppState>>,
) -> Result<()> {
    let dest = Path::new("/Volumes/video/RawRips");
    
    // Check if destination exists
    if !dest.exists() {
        let msg = format!("⚠️  Destination {} does not exist - skipping rsync", dest.display());
        warn!("{}", msg);
        let mut state = tui_state.lock().await;
        state.add_rsync_log(msg);
        return Err(anyhow::anyhow!("Destination directory does not exist"));
    }
    
    let source_name = source_dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    
    let msg = format!("🚀 Starting rsync: {} → /Volumes/video/RawRips/", source_name);
    info!("{}", msg);
    {
        let mut state = tui_state.lock().await;
        state.add_rsync_log(msg);
    }
    
    // Use rsync with progress and verbose output
    // -a: archive mode (recursive, preserve permissions, etc.)
    // -v: verbose
    // --progress: show progress (macOS rsync compatible)
    // Note: macOS rsync doesn't support --info=progress2 (requires rsync 3.1+)
    let mut child = Command::new("rsync")
        .arg("-av")
        .arg("--progress")
        .arg(format!("{}/", source_dir.display())) // Trailing slash = copy contents
        .arg(format!("{}/{}/", dest.display(), source_name))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    
    let tui_state_clone = Arc::clone(&tui_state);
    
    // Handle stdout
    let stdout_handle = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        
        while let Ok(Some(line)) = lines.next_line().await {
            debug!("rsync stdout: {}", line);
            
            // Filter out noise and format useful lines
            if line.trim().is_empty() {
                continue;
            }
            
            // Progress lines look like: "1,234,567,890  45%  123.45MB/s    0:00:12"
            if line.contains('%') {
                let mut state = tui_state_clone.lock().await;
                state.add_rsync_log(format!("📊 {}", line.trim()));
            } else if line.starts_with("sending incremental") {
                let mut state = tui_state_clone.lock().await;
                state.add_rsync_log("📡 Calculating differences...".to_string());
            } else if line.starts_with("sent") || line.starts_with("total size") {
                let mut state = tui_state_clone.lock().await;
                state.add_rsync_log(format!("ℹ️  {}", line.trim()));
            }
        }
    });
    
    // Handle stderr
    let tui_state_err = Arc::clone(&tui_state);
    let stderr_handle = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                warn!("rsync stderr: {}", line);
                let mut state = tui_state_err.lock().await;
                state.add_rsync_log(format!("⚠️  {}", line.trim()));
            }
        }
    });
    
    // Wait for process to complete
    let status = child.wait().await?;
    
    // Wait for output handlers to finish
    let _ = tokio::join!(stdout_handle, stderr_handle);
    
    if status.success() {
        let msg = format!("✅ Rsync complete: {}", source_name);
        info!("{}", msg);
        let mut state = tui_state.lock().await;
        state.add_rsync_log(msg);
        Ok(())
    } else {
        let msg = format!("❌ Rsync failed with exit code: {:?}", status.code());
        warn!("{}", msg);
        let mut state = tui_state.lock().await;
        state.add_rsync_log(msg);
        Err(anyhow::anyhow!("Rsync failed"))
    }
}

/// Web-UI-only rsync function (no TUI, all logs go to web UI)
pub async fn rsync_to_rawrips_web_ui(
    source_dir: &Path,
    state: &ApiState,
    device: &str,
    operation_id: Option<&str>,
) -> Result<()> {
    // Helper to send logs to web UI
    async fn send_log(
        state: &ApiState,
        drive: &str,
        level: &str,
        message: String,
        operation_id: Option<&str>,
    ) {
        let _ = state.event_tx.send(crate::api::ApiEvent::Log {
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
    
    let dest = Path::new("/Volumes/video/RawRips");
    
    // Check if destination exists
    if !dest.exists() {
        let msg = format!("⚠️  Destination {} does not exist - skipping rsync", dest.display());
        warn!("{}", msg);
        send_log(state, device, "warning", msg, operation_id).await;
        return Err(anyhow::anyhow!("Destination directory does not exist"));
    }
    
    let source_name = source_dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    
    let msg = format!("🚀 Starting rsync: {} → /Volumes/video/RawRips/", source_name);
    info!("{}", msg);
    send_log(state, device, "info", msg, operation_id).await;
    
    // Use rsync with progress and verbose output
    let mut child = Command::new("rsync")
        .arg("-av")
        .arg("--progress")
        .arg(format!("{}/", source_dir.display()))
        .arg(format!("{}/{}/", dest.display(), source_name))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    
    let state_clone = state.clone();
    let device_clone = device.to_string();
    let operation_id_clone = operation_id.map(|s| s.to_string());
    
    // Handle stdout
    let stdout_handle = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        
        while let Ok(Some(line)) = lines.next_line().await {
            debug!("rsync stdout: {}", line);
            
            if line.trim().is_empty() {
                continue;
            }
            
            // Progress lines look like: "1,234,567,890  45%  123.45MB/s    0:00:12"
            if line.contains('%') {
                send_log(&state_clone, &device_clone, "info", format!("📊 {}", line.trim()), operation_id_clone.as_deref()).await;
            } else if line.starts_with("sending incremental") {
                send_log(&state_clone, &device_clone, "info", "📡 Calculating differences...".to_string(), operation_id_clone.as_deref()).await;
            } else if line.starts_with("sent") || line.starts_with("total size") {
                send_log(&state_clone, &device_clone, "info", format!("ℹ️  {}", line.trim()), operation_id_clone.as_deref()).await;
            }
        }
    });
    
    // Handle stderr
    let state_err = state.clone();
    let device_err = device.to_string();
    let operation_id_err = operation_id.map(|s| s.to_string());
    let stderr_handle = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                warn!("rsync stderr: {}", line);
                send_log(&state_err, &device_err, "warning", format!("⚠️  {}", line.trim()), operation_id_err.as_deref()).await;
            }
        }
    });
    
    // Wait for process to complete
    let status = child.wait().await?;
    
    // Wait for output handlers to finish
    let _ = tokio::join!(stdout_handle, stderr_handle);
    
    if status.success() {
        let msg = format!("✅ Rsync complete: {}", source_name);
        info!("{}", msg);
        send_log(state, device, "success", msg, operation_id).await;
        Ok(())
    } else {
        let msg = format!("❌ Rsync failed with exit code: {:?}", status.code());
        warn!("{}", msg);
        send_log(state, device, "error", msg, operation_id).await;
        Err(anyhow::anyhow!("Rsync failed"))
    }
}
