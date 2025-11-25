use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "ripley")]
#[command(author = "Johnny")]
#[command(version = "0.1.0")]
#[command(about = "Automated CD ripper with real-time TUI", long_about = None)]
pub struct Args {
    /// Output folder for ripped files
    #[arg(short, long, value_name = "DIR")]
    pub output_folder: Option<PathBuf>,

    /// FLAC compression quality (0-8, default: 5)
    #[arg(short, long, default_value = "5")]
    pub quality: u8,

    /// Automatically eject disc when ripping completes
    #[arg(short, long, default_value = "true")]
    pub eject_when_done: bool,

    /// Skip metadata fetching (offline mode)
    #[arg(short, long, default_value = "false")]
    pub skip_metadata: bool,
}

impl Args {
    pub fn get_output_folder(&self) -> PathBuf {
        self.output_folder.clone().unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join("Desktop").join("Rips").join("Music")
        })
    }
}
