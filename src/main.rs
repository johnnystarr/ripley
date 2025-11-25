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
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Parse CLI arguments
    let args = Args::parse();

    // Run the application
    app::run(args).await?;

    Ok(())
}
