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
    // Set up comprehensive file logging to ~/.config/ripley/agent.log
    let log_dir = dirs::home_dir()
        .map(|h| h.join(".config").join("ripley"))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    std::fs::create_dir_all(&log_dir)?;
    
    let log_file = log_dir.join("agent.log");
    
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)?;
    
    // Initialize logging to both file and console with DEBUG level
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(std::sync::Arc::new(file))
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .init();
    
    tracing::info!("🤖 Ripley Agent - Logging to: {}", log_file.display());

    // Load configuration
    let config = AgentConfig::load()?;
    
    // Create TUI app (will handle connection in UI)
    let mut app = TuiApp::new(config)?;
    
    // Run TUI
    app.run().await?;
    
    Ok(())
}

