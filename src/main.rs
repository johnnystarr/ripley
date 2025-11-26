mod cli;
mod config;
mod drive;
mod dvd_metadata;
mod dvd_ripper;
mod metadata;
mod ripper;
mod audio;
mod tui;
mod app;
mod notifications;
mod filebot;
mod rsync;
mod speech_match;
mod rename;

use anyhow::Result;
use cli::{Args, Command};
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

    // Handle subcommands
    match &args.command {
        Some(Command::Rename {
            directory,
            title,
            skip_speech,
            skip_filebot,
        }) => {
            // Run rename command
            rename::run_rename(
                directory.clone(),
                title.clone(),
                *skip_speech,
                *skip_filebot,
            )
            .await?;
        }
        None => {
            // No subcommand = default rip behavior
            app::run(args).await?;
        }
    }

    Ok(())
}
