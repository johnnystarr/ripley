use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub server_url: String,
    pub agent_name: String,
    pub agent_id: Option<String>,
    pub api_key: Option<String>,
    pub heartbeat_interval_seconds: u64,
    pub instruction_poll_interval_seconds: u64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:3000".to_string(),
            agent_name: format!("agent-{}", std::env::var("COMPUTERNAME")
                .unwrap_or_else(|_| "unknown".to_string())),
            agent_id: None,
            api_key: None,
            heartbeat_interval_seconds: 30,
            instruction_poll_interval_seconds: 5,
        }
    }
}

impl AgentConfig {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path();
        
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: AgentConfig = toml::from_str(&content)?;
            info!("Loaded config from {:?}", config_path);
            Ok(config)
        } else {
            let config = Self::default();
            config.save()?;
            info!("Created default config at {:?}", config_path);
            Ok(config)
        }
    }
    
    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path();
        
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        
        Ok(())
    }
    
    fn get_config_path() -> PathBuf {
        if let Some(home) = dirs::home_dir() {
            home.join(".config").join("ripley-agent").join("config.toml")
        } else {
            PathBuf::from("config.toml")
        }
    }
}

