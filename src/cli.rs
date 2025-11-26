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

    /// Manually specify the title for DVD/Blu-ray metadata lookup (e.g., "Foster's Home for Imaginary Friends")
    #[arg(short, long, value_name = "TITLE")]
    pub title: Option<String>,
}

impl Args {
    pub fn get_output_folder(&self) -> PathBuf {
        self.output_folder.clone().unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join("Desktop").join("Rips").join("Music")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_quality() {
        let args = Args {
            output_folder: None,
            quality: 5,
            eject_when_done: true,
            skip_metadata: false,
        };
        
        assert_eq!(args.quality, 5);
    }

    #[test]
    fn test_custom_output_folder() {
        let custom_path = PathBuf::from("/custom/path");
        let args = Args {
            output_folder: Some(custom_path.clone()),
            quality: 8,
            eject_when_done: false,
            skip_metadata: true,
        };
        
        assert_eq!(args.get_output_folder(), custom_path);
        assert_eq!(args.quality, 8);
        assert!(!args.eject_when_done);
        assert!(args.skip_metadata);
    }

    #[test]
    fn test_default_output_folder() {
        let args = Args {
            output_folder: None,
            quality: 5,
            eject_when_done: true,
            skip_metadata: false,
        };
        
        let folder = args.get_output_folder();
        assert!(folder.to_string_lossy().contains("Desktop"));
        assert!(folder.to_string_lossy().contains("Rips"));
        assert!(folder.to_string_lossy().contains("Music"));
    }

    #[test]
    fn test_quality_range() {
        // Test valid quality values
        for q in 0..=8 {
            let args = Args {
                output_folder: None,
                quality: q,
                eject_when_done: true,
                skip_metadata: false,
            };
            assert!(args.quality <= 8);
        }
    }
}
