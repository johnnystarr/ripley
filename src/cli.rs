use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "ripley")]
#[command(author = "Johnny")]
#[command(version = "0.1.0")]
#[command(about = "Automated Optical Disc Ripper with real-time TUI", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Output folder for ripped files
    #[arg(short, long, value_name = "DIR", global = true)]
    pub output_folder: Option<PathBuf>,

    /// Skip metadata fetching (offline mode)
    #[arg(short, long, default_value = "false", global = true)]
    pub skip_metadata: bool,

    /// Manually specify the title for DVD/Blu-ray metadata lookup
    #[arg(short, long, value_name = "TITLE", global = true)]
    pub title: Option<String>,

    /// Skip Filebot renaming (Filebot runs by default)
    #[arg(long, default_value = "false", global = true)]
    pub skip_filebot: bool,
    
    // Legacy args for backward compatibility
    /// FLAC compression quality (0-8, default: 5)
    #[arg(short, long, default_value = "5", hide = true)]
    pub quality: u8,

    /// Automatically eject disc when ripping completes
    #[arg(short, long, default_value = "true", hide = true)]
    pub eject_when_done: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Rename existing video files using speech matching + Filebot
    Rename {
        /// Directory containing video files to rename (defaults to current directory)
        #[arg(value_name = "DIR")]
        directory: Option<PathBuf>,
        
        /// TV show title (will prompt if not provided)
        #[arg(short, long, value_name = "TITLE")]
        title: Option<String>,
        
        /// Skip speech matching phase (only use Filebot duration matching)
        #[arg(long)]
        skip_speech: bool,
        
        /// Skip Filebot phase (only use speech matching)
        #[arg(long)]
        skip_filebot: bool,
    },
}

impl Args {
    pub fn get_output_folder(&self) -> PathBuf {
        self.output_folder.clone().unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join("Desktop").join("Rips").join("Music")
        })
    }
    
    /// Check if this is the rename subcommand
    pub fn is_rename_command(&self) -> bool {
        matches!(self.command, Some(Command::Rename { .. }))
    }
}
