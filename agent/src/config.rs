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
        let default_name = if cfg!(target_os = "windows") {
            std::env::var("COMPUTERNAME")
                .unwrap_or_else(|_| "windows-agent".to_string())
        } else {
            std::env::var("HOSTNAME")
                .unwrap_or_else(|_| "agent".to_string())
        };
        
        Self {
            server_url: String::new(), // Empty by default - user must enter
            agent_name: default_name,
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
            let config: AgentConfig = serde_yaml::from_str(&content)?;
            info!("Loaded config from {:?}", config_path);
            Ok(config)
        } else {
            let config = Self::default();
            // Don't save empty config - wait for user to enter values
            Ok(config)
        }
    }
    
    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path();
        
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = serde_yaml::to_string(self)?;
        std::fs::write(&config_path, content)?;
        info!("Saved config to {:?}", config_path);
        
        Ok(())
    }
    
    fn get_config_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                PathBuf::from(appdata).join("ripley-agent").join("agent.yaml")
            } else {
                PathBuf::from("agent.yaml")
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            if let Some(home) = dirs::home_dir() {
                home.join(".config").join("ripley").join("agent.yaml")
            } else {
                PathBuf::from("agent.yaml")
            }
        }
        
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            if let Some(home) = dirs::home_dir() {
                home.join(".config").join("ripley").join("agent.yaml")
            } else {
                PathBuf::from("agent.yaml")
            }
        }
    }
}

