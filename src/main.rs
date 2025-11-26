mod cli;
mod drive;
mod metadata;
mod ripper;
mod audio;
mod tui;
mod app;

use anyhow::Result;
use cli::Args;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize file logging to ~/ripley.log
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let log_path = std::path::PathBuf::from(home).join("ripley.log");
    
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)?;
    
    tracing_subscriber::fmt()
        .with_writer(std::sync::Arc::new(file))
        .with_ansi(false)
        .init();
    
    eprintln!("📝 Logging to: {}", log_path.display());

    // Parse CLI arguments
    let args = Args::parse();

    // Run the application
    app::run(args).await?;

    Ok(())
}
