use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "ripley")]
#[command(author = "Johnny Staryavsky")]
#[command(version = "0.1.0")]
#[command(about = "🎬 Automated Optical Disc Ripper with AI-powered episode matching")]
#[command(long_about = "Ripley automatically rips DVDs and Blu-rays with intelligent episode identification.\n\
    \n\
    Features:\n\
      • Real-time TUI with multi-drive support\n\
      • AI-powered episode matching via subtitles + GPT-4o\n\
      • Automatic metadata from TMDB + TheTVDB\n\
      • Smart renaming with Filebot integration\n\
      • Background rsync to network storage\n\
      • macOS notifications on completion\n\
    \n\
    Run 'ripley rip' to start the interactive ripper.\n\
    Run 'ripley rename' to batch-rename existing files.")]
#[command(styles = get_styles())]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// 💿 Start the interactive disc ripper (monitors optical drives)
    #[command(visible_alias = "r")]
    Rip {
        /// Output directory for ripped files
        #[arg(short, long, value_name = "DIR")]
        #[arg(help = "Where to save ripped files (default: ~/Desktop/Rips/Video)")]
        output_folder: Option<PathBuf>,

        /// Manually specify TV show title for metadata lookup
        #[arg(short, long, value_name = "TITLE")]
        #[arg(help = "Override disc title (e.g., 'Star Trek TNG')")]
        title: Option<String>,

        /// Skip metadata fetching from TMDB (offline mode)
        #[arg(long)]
        #[arg(help = "Don't fetch episode info from TMDB")]
        skip_metadata: bool,

        /// Skip Filebot renaming step
        #[arg(long)]
        #[arg(help = "Don't run Filebot after speech matching")]
        skip_filebot: bool,
    },

    /// 📝 Rename existing video files using AI episode matching + Filebot
    #[command(visible_alias = "rn")]
    Rename {
        /// Directory containing video files to rename
        #[arg(value_name = "DIR")]
        #[arg(help = "Path to folder with video files (default: current directory)")]
        directory: Option<PathBuf>,
        
        /// TV show title for metadata lookup
        #[arg(short, long, value_name = "TITLE")]
        #[arg(help = "TV show name (e.g., 'The Office')")]
        title: Option<String>,
        
        /// Skip speech/subtitle matching phase
        #[arg(long)]
        #[arg(help = "Only use Filebot duration matching")]
        skip_speech: bool,
        
        /// Skip Filebot phase
        #[arg(long)]
        #[arg(help = "Only use speech/subtitle matching")]
        skip_filebot: bool,
    },

    /// 🌐 Start REST API server for remote control (for web UI)
    #[command(visible_alias = "api")]
    Serve {
        /// Port to listen on
        #[arg(short, long, value_name = "PORT", default_value = "3000")]
        #[arg(help = "HTTP port for API server")]
        port: u16,

        /// Host address to bind to
        #[arg(long, value_name = "HOST", default_value = "127.0.0.1")]
        #[arg(help = "Network interface to bind (use 0.0.0.0 for all)")]
        host: String,

        /// Development mode (don't serve embedded UI, use Vite dev server)
        #[arg(long)]
        #[arg(help = "Enable dev mode for UI hot reload")]
        dev: bool,
    },
}

fn get_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .usage(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Cyan)))
        )
        .header(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow)))
        )
        .literal(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green)))
        )
        .placeholder(
            anstyle::Style::new()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Cyan)))
        )
        .valid(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green)))
        )
}

// Legacy struct for backward compatibility with app.rs
#[derive(Debug, Clone)]
pub struct RipArgs {
    pub output_folder: Option<PathBuf>,
    pub skip_metadata: bool,
    pub title: Option<String>,
    pub skip_filebot: bool,
    // Legacy audio CD fields
    pub quality: u8,
    pub eject_when_done: bool,
}

impl RipArgs {
    pub fn get_output_folder(&self) -> PathBuf {
        self.output_folder.clone().unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join("Desktop").join("Rips").join("Video")
        })
    }
}


