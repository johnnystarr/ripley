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
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .init();

    // Load configuration
    let config = AgentConfig::load()?;
    
    // Create TUI app (will handle connection in UI)
    let mut app = TuiApp::new(config)?;
    
    // Run TUI
    app.run().await?;
    
    Ok(())
}

