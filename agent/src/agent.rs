use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};
use crate::config::AgentConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistration {
    pub agent_id: String,
    pub name: String,
    pub platform: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instruction {
    pub id: i64,
    pub instruction_type: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub assigned_to_agent_id: Option<String>,
}

#[derive(Clone)]
pub struct AgentClient {
    config: AgentConfig,
    http_client: reqwest::Client,
    agent_id: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

impl AgentClient {
    pub fn new(config: AgentConfig) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        Ok(Self {
            config,
            http_client,
            agent_id: std::sync::Arc::new(std::sync::Mutex::new(None)),
        })
    }
    
    pub async fn register(&self) -> Result<()> {
        let platform = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        };
        
        let registration = serde_json::json!({
            "name": self.config.agent_name,
            "platform": platform,
            "capabilities": {
                "topaz_video": self.check_topaz_installed().await,
            }
        });
        
        let url = format!("{}/api/agents/register", self.config.server_url);
        let response = self.http_client
            .post(&url)
            .json(&registration)
            .send()
            .await?;
        
        if response.status().is_success() {
            let result: serde_json::Value = response.json().await?;
            let agent_id = result.get("agent_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            
            if let Some(ref id) = agent_id {
                info!("Agent registered successfully: {}", id);
                *self.agent_id.lock().unwrap() = agent_id.clone();
            }
            Ok(())
        } else {
            let error_text = response.text().await?;
            Err(anyhow::anyhow!("Registration failed: {}", error_text))
        }
    }
    
    pub async fn heartbeat(&self) -> Result<()> {
        let agent_id = self.agent_id.lock().unwrap().clone();
        if let Some(ref agent_id) = agent_id {
            let url = format!("{}/api/agents/{}/heartbeat", self.config.server_url, agent_id);
            let response = self.http_client
                .post(&url)
                .json(&serde_json::json!({}))
                .send()
                .await?;
            
            if !response.status().is_success() {
                warn!("Heartbeat failed: {}", response.status());
            }
        }
        Ok(())
    }
    
    pub async fn get_instructions(&self) -> Result<Vec<Instruction>> {
        let agent_id = self.agent_id.lock().unwrap().clone();
        if let Some(ref agent_id) = agent_id {
            let url = format!("{}/api/agents/{}/instructions", self.config.server_url, agent_id);
            let response = self.http_client
                .get(&url)
                .send()
                .await?;
            
            if response.status().is_success() {
                let instructions: Vec<Instruction> = response.json().await?;
                Ok(instructions)
            } else {
                Ok(vec![])
            }
        } else {
            Ok(vec![])
        }
    }
    
    pub async fn check_topaz_installed(&self) -> bool {
        // Check if Topaz Video AI is installed
        // On Windows, typically in Program Files
        if cfg!(target_os = "windows") {
            let possible_paths = vec![
                r"C:\Program Files\Topaz Labs LLC\Topaz Video AI\Topaz Video AI.exe",
                r"C:\Program Files (x86)\Topaz Labs LLC\Topaz Video AI\Topaz Video AI.exe",
            ];
            
            for path in possible_paths {
                if std::path::Path::new(path).exists() {
                    return true;
                }
            }
        }
        false
    }
    
    pub fn agent_id(&self) -> Option<String> {
        self.agent_id.lock().unwrap().clone()
    }
}

