use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::time::Duration;
use tokio::time;
use serde::{Deserialize, Serialize};

mod agent;
mod config;
mod tui;

use agent::AgentClient;
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
    
    // Create agent client
    let agent_client = AgentClient::new(config.clone())?;
    
    // Register agent
    agent_client.register().await?;
    
    // Create TUI app
    let mut app = TuiApp::new(agent_client, config)?;
    
    // Run TUI
    app.run().await?;
    
    Ok(())
}

