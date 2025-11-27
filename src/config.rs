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
    pub retry: RetryConfig,
    pub rip_profiles: Vec<RipProfile>,
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
    pub use_for_music: bool,  // Whether to use Filebot to standardize music filenames
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub enabled: bool,
    pub max_attempts: u32,
    pub initial_delay_seconds: u64,
    pub max_delay_seconds: u64,
    pub backoff_multiplier: f64,
}

/// Rip quality profile for different use cases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipProfile {
    pub name: String,
    pub description: Option<String>,
    pub audio_quality: Option<u8>, // 0-9 for FLAC quality (audio CDs)
    pub makemkv_profile: Option<String>, // MakeMKV profile name (DVDs/Blu-rays)
    pub is_default: bool,
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
                use_for_music: true,  // Enable Filebot for music standardization by default
            },
            retry: RetryConfig {
                enabled: true,
                max_attempts: 3,
                initial_delay_seconds: 1,
                max_delay_seconds: 60,
                backoff_multiplier: 2.0,
            },
            rip_profiles: vec![
                RipProfile {
                    name: "High Quality".to_string(),
                    description: Some("Best quality, larger file sizes".to_string()),
                    audio_quality: Some(8),
                    makemkv_profile: Some("default".to_string()),
                    is_default: false,
                },
                RipProfile {
                    name: "Standard".to_string(),
                    description: Some("Balanced quality and file size".to_string()),
                    audio_quality: Some(5),
                    makemkv_profile: Some("default".to_string()),
                    is_default: true,
                },
                RipProfile {
                    name: "Fast".to_string(),
                    description: Some("Faster ripping, smaller files".to_string()),
                    audio_quality: Some(3),
                    makemkv_profile: Some("default".to_string()),
                    is_default: false,
                },
            ],
        }
    }
}

/// Get the path to the config file (tries project root first, then ~/.config/ripley/config.yaml)
pub fn get_config_path() -> PathBuf {
    // Try project root first
    let project_config = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.yaml");
    
    if project_config.exists() {
        return project_config;
    }
    
    // Try home config dir
    if let Some(home_dir) = dirs::home_dir() {
        let home_config = home_dir.join(".config").join("ripley").join("config.yaml");
        if home_config.exists() {
            return home_config;
        }
    }
    
    // Default to project root even if it doesn't exist
    project_config
}

impl Config {
    /// Load config from config.yaml in the project root or ~/.config/ripley/config.yaml
    pub fn load() -> Result<Self> {
        let config_path = get_config_path();
        
        if config_path.exists() {
            info!("Loading config from {}", config_path.display());
            Self::load_from_file(&config_path)
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
    #[allow(dead_code)]
    pub fn get_tmdb_api_key(&self) -> Option<String> {
        self.tmdb_api_key.clone()
    }
    
    /// Get the default rip profile
    pub fn get_default_profile(&self) -> Option<&RipProfile> {
        self.rip_profiles.iter()
            .find(|p| p.is_default)
            .or_else(|| self.rip_profiles.first())
    }
    
    /// Get a rip profile by name
    pub fn get_profile(&self, name: &str) -> Option<&RipProfile> {
        self.rip_profiles.iter().find(|p| p.name == name)
    }
}

/// Save config to the default config file
pub fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path();
    let contents = serde_yaml::to_string(config)?;
    std::fs::write(config_path, contents)?;
    info!("Configuration saved");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert!(config.openai_api_key.is_none());
        assert!(config.tmdb_api_key.is_some());
        assert_eq!(config.speech_match.audio_duration, 180);
        assert_eq!(config.filebot.database, "TheTVDB");
        assert_eq!(config.notifications.enabled, true);
        assert_eq!(config.rsync.enabled, true);
    }

    #[test]
    fn test_config_with_api_keys() {
        let config = Config {
            openai_api_key: Some("test_openai_key".to_string()),
            tmdb_api_key: Some("test_tmdb_key".to_string()),
            ..Default::default()
        };

        assert_eq!(config.openai_api_key, Some("test_openai_key".to_string()));
        assert_eq!(config.tmdb_api_key, Some("test_tmdb_key".to_string()));
    }

    #[test]
    fn test_speech_match_config() {
        let speech_config = SpeechMatchConfig {
            enabled: true,
            audio_duration: 240,
            whisper_model: "base".to_string(),
            use_openai_api: true,
        };

        assert_eq!(speech_config.audio_duration, 240);
        assert_eq!(speech_config.enabled, true);
        assert_eq!(speech_config.whisper_model, "base");
    }

    #[test]
    fn test_filebot_config() {
        let filebot_config = FilebotConfig {
            skip_by_default: false,
            database: "TVDB".to_string(),
            order: "Airdate".to_string(),
            use_for_music: true,
        };

        assert_eq!(filebot_config.database, "TVDB");
        assert_eq!(filebot_config.order, "Airdate");
        assert_eq!(filebot_config.skip_by_default, false);
        assert_eq!(filebot_config.use_for_music, true);
    }

    #[test]
    fn test_config_get_openai_key() {
        let config = Config {
            openai_api_key: Some("my_key".to_string()),
            ..Default::default()
        };

        assert_eq!(config.get_openai_api_key(), Some("my_key".to_string()));
    }

    #[test]
    fn test_config_get_tmdb_key() {
        let config = Config {
            tmdb_api_key: Some("tmdb_key".to_string()),
            ..Default::default()
        };

        assert_eq!(config.get_tmdb_api_key(), Some("tmdb_key".to_string()));
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            openai_api_key: Some("test_key".to_string()),
            tmdb_api_key: Some("tmdb_test".to_string()),
            speech_match: SpeechMatchConfig {
                enabled: true,
                audio_duration: 200,
                whisper_model: "base".to_string(),
                use_openai_api: true,
            },
            filebot: FilebotConfig {
                skip_by_default: false,
                database: "Custom".to_string(),
                order: "Airdate".to_string(),
                use_for_music: true,
            },
            ..Default::default()
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("openai_api_key"));
        assert!(yaml.contains("test_key"));
        assert!(yaml.contains("audio_duration: 200"));
        assert!(yaml.contains("database: Custom"));
    }

    #[test]
    fn test_config_deserialization() {
        let yaml = r#"
openai_api_key: my_openai_key
tmdb_api_key: my_tmdb_key
notifications:
  enabled: true
  topic: test_topic
rsync:
  enabled: true
  destination: /test/path
speech_match:
  enabled: true
  audio_duration: 150
  whisper_model: base
  use_openai_api: true
filebot:
  skip_by_default: false
  database: TheTVDB
  order: Airdate
  use_for_music: true
retry:
  enabled: true
  max_attempts: 3
  initial_delay_seconds: 1
  max_delay_seconds: 60
  backoff_multiplier: 2.0
rip_profiles: []
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.openai_api_key, Some("my_openai_key".to_string()));
        assert_eq!(config.tmdb_api_key, Some("my_tmdb_key".to_string()));
        assert_eq!(config.speech_match.audio_duration, 150);
        assert_eq!(config.filebot.database, "TheTVDB");
    }
}
