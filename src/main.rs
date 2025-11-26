mod api;
mod cli;
mod config;
mod database;
mod drive;
mod web_ui;
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
use cli::{Args, Command, RipArgs};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Handle subcommands
    match &args.command {
        Some(Command::Rip {
            output_folder,
            title,
            skip_metadata,
            skip_filebot,
        }) => {
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
            
            eprintln!("\x1b[36m📝 Logging to:\x1b[0m {}", log_path.display());
            eprintln!("\x1b[32m🎬 Starting Ripley disc ripper...\x1b[0m\n");

            // Convert to RipArgs for app.rs
            let rip_args = RipArgs {
                output_folder: output_folder.clone(),
                skip_metadata: *skip_metadata,
                title: title.clone(),
                skip_filebot: *skip_filebot,
                quality: 5,  // Default FLAC quality for audio CDs
                eject_when_done: true,  // Default eject behavior
            };
            
            // Run the ripper
            app::run(rip_args).await?;
        }
        Some(Command::Rename {
            directory,
            title,
            skip_speech,
            skip_filebot,
        }) => {
            eprintln!("\x1b[35m📝 Ripley Rename Tool\x1b[0m");
            eprintln!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m\n");
            
            // Run rename command
            rename::run_rename(
                directory.clone(),
                title.clone(),
                *skip_speech,
                *skip_filebot,
            )
            .await?;
        }
        Some(Command::Serve { port, host, dev }) => {
            eprintln!("\x1b[35m🌐 Ripley REST API Server\x1b[0m");
            eprintln!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m\n");
            
            // Load configuration
            let config = config::Config::load()?;
            
            // Start API server
            api::start_server(config, host.clone(), *port, *dev).await?;
        }
        None => {
            // No subcommand provided - print help with a friendly message
            eprintln!("\x1b[33m⚠️  No command specified.\x1b[0m\n");
            eprintln!("\x1b[36mℹ️  Ripley is a disc ripper with AI-powered episode matching.\x1b[0m");
            eprintln!("\x1b[36m   Run '\x1b[1;32mripley --help\x1b[0;36m' to see all available commands.\x1b[0m\n");
            eprintln!("\x1b[1mQuick Start:\x1b[0m");
            eprintln!("  \x1b[32mripley rip\x1b[0m       Start the interactive disc ripper");
            eprintln!("  \x1b[32mripley rename\x1b[0m    Rename existing video files");
            eprintln!("  \x1b[32mripley serve\x1b[0m     Start REST API server\n");
            
            std::process::exit(1);
        }
    }

    Ok(())
}
