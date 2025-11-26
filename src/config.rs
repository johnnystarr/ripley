use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub openai_api_key: Option<String>,
    pub tmdb_api_key: Option<String>,
    pub notifications: NotificationConfig,
    pub rsync: RsyncConfig,
    pub speech_match: SpeechMatchConfig,
    pub filebot: FilebotConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub topic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RsyncConfig {
    pub enabled: bool,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechMatchConfig {
    pub enabled: bool,
    pub audio_duration: u32,
    pub whisper_model: String,
    pub use_openai_api: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilebotConfig {
    pub skip_by_default: bool,
    pub database: String,
    pub order: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            openai_api_key: None,
            tmdb_api_key: Some("fef1285fb85a74350b3292b5fac37fce".to_string()),
            notifications: NotificationConfig {
                enabled: true,
                topic: "staryavsky_alerts".to_string(),
            },
            rsync: RsyncConfig {
                enabled: true,
                destination: "/Volumes/video/RawRips".to_string(),
            },
            speech_match: SpeechMatchConfig {
                enabled: true,
                audio_duration: 180,  // 3 minutes for better accuracy
                whisper_model: "base".to_string(),
                use_openai_api: true,
            },
            filebot: FilebotConfig {
                skip_by_default: false,
                database: "TheTVDB".to_string(),
                order: "Airdate".to_string(),
            },
        }
    }
}

impl Config {
    /// Load config from config.yaml in the project root or ~/.config/ripley/config.yaml
    pub fn load() -> Result<Self> {
        // Try project root first
        let project_config = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.yaml");
        
        // Try home config dir
        let home_config = dirs::home_dir()
            .map(|h| h.join(".config").join("ripley").join("config.yaml"));
        
        let config_path = if project_config.exists() {
            Some(project_config)
        } else if let Some(ref path) = home_config {
            if path.exists() {
                Some(path.clone())
            } else {
                None
            }
        } else {
            None
        };
        
        if let Some(path) = config_path {
            info!("Loading config from {}", path.display());
            Self::load_from_file(&path)
        } else {
            warn!("No config.yaml found, using defaults");
            Ok(Config::default())
        }
    }
    
    /// Load config from specific file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let mut config: Config = serde_yaml::from_str(&contents)?;
        
        // Validate API keys
        if let Some(ref key) = config.openai_api_key {
            if key == "YOUR_API_KEY_HERE" || key.is_empty() {
                warn!("OpenAI API key not configured in config.yaml");
                config.openai_api_key = None;
            }
        }
        
        debug!("Config loaded: speech_match={}, filebot={}", 
               config.speech_match.enabled, !config.filebot.skip_by_default);
        
        Ok(config)
    }
    
    /// Get OpenAI API key from config or environment variable
    pub fn get_openai_api_key(&self) -> Option<String> {
        self.openai_api_key.clone()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
    }
    
    /// Get TMDB API key from config
    pub fn get_tmdb_api_key(&self) -> Option<String> {
        self.tmdb_api_key.clone()
    }
}
