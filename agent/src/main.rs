use anyhow::Result;

mod agent;
mod config;
mod tui;
mod topaz;
mod job_worker;

use config::AgentConfig;
use tui::TuiApp;

#[tokio::main]
async fn main() -> Result<()> {
    // Set up file logging
    let log_dir = dirs::home_dir()
        .map(|h| h.join(".cache").join("ripley-agent").join("logs"))
        .unwrap_or_else(|| std::path::PathBuf::from("logs"));
    std::fs::create_dir_all(&log_dir)?;
    
    let log_file = log_dir.join(format!("ripley-agent-{}.log", 
        chrono::Local::now().format("%Y%m%d")));
    
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)?;
    
    // Initialize logging to both file and console
    tracing_subscriber::fmt()
        .with_target(false)
        .with_writer(std::sync::Arc::new(file))
        .with_ansi(false)
        .init();
    
    tracing::info!("Logging to: {}", log_file.display());

    // Load configuration
    let config = AgentConfig::load()?;
    
    // Create TUI app (will handle connection in UI)
    let mut app = TuiApp::new(config)?;
    
    // Run TUI
    app.run().await?;
    
    Ok(())
}

