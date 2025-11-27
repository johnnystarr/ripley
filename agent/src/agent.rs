use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};
use crate::config::AgentConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
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
            config: config.clone(),
            http_client,
            agent_id: std::sync::Arc::new(std::sync::Mutex::new(None)),
        })
    }
    
    #[allow(dead_code)]
    pub fn update_server_url(&mut self, server_url: String) {
        self.config.server_url = server_url;
    }
    
    pub async fn register(&self) -> Result<()> {
        let platform = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        };
        
        // Check Topaz capabilities
        let (topaz_installed, topaz_version) = self.check_topaz_capabilities().await;
        let capabilities = serde_json::json!({
            "topaz_video": topaz_installed,
        });
        
        // Generate agent_id if not set
        let agent_id = self.config.agent_id.clone().unwrap_or_else(|| {
            format!("agent-{}", std::env::var("COMPUTERNAME")
                .unwrap_or_else(|_| std::env::var("HOSTNAME")
                    .unwrap_or_else(|_| "unknown".to_string())))
        });
        
        let registration = serde_json::json!({
            "agent_id": agent_id,
            "name": self.config.agent_name,
            "platform": platform,
            "capabilities": serde_json::to_string(&capabilities).unwrap_or_else(|_| "{}".to_string()),
            "topaz_version": topaz_version,
            "api_key": self.config.api_key.clone(),
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
    
    /// Check Topaz capabilities (installed status and version)
    pub async fn check_topaz_capabilities(&self) -> (bool, Option<String>) {
        if cfg!(target_os = "windows") {
            if let Some(topaz_path) = crate::topaz::TopazVideo::find_executable() {
                // Try to get version from executable
                let version = self.get_topaz_version(&topaz_path).await;
                return (true, version);
            }
        }
        (false, None)
    }
    
    /// Get Topaz Video version from executable
    async fn get_topaz_version(&self, exe_path: &std::path::Path) -> Option<String> {
        // Try simple approach: check if TopazVideo can get version
        match crate::topaz::TopazVideo::new() {
            Ok(topaz) => {
                // Try to get version (if implemented)
                match topaz.get_version().await {
                    Ok(version) if version != "Unknown" => Some(version),
                    _ => {
                        // Fallback: use executable filename/path as identifier
                        exe_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| format!("Topaz Video AI ({})", s))
                            .or_else(|| Some("Installed".to_string()))
                    }
                }
            }
            Err(_) => None,
        }
    }
    
    #[allow(dead_code)]
    pub async fn check_topaz_installed(&self) -> bool {
        self.check_topaz_capabilities().await.0
    }
    
    pub fn agent_id(&self) -> Option<String> {
        self.agent_id.lock().unwrap().clone()
    }
    
    /// Get next available upscaling job
    pub async fn get_next_upscaling_job(&self) -> Result<Option<UpscalingJob>> {
        let url = format!("{}/api/upscaling-jobs/next", self.config.server_url);
        let response = self.http_client
            .get(&url)
            .send()
            .await?;
        
        if response.status().is_success() {
            let job: Option<UpscalingJob> = response.json().await?;
            Ok(job)
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            Err(anyhow::anyhow!("Failed to get next upscaling job: {}", response.status()))
        }
    }
    
    /// Download file from server
    pub async fn download_file(&self, file_path: &str, dest_path: &std::path::Path) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        
        // URL encode the file path
        let encoded_path = urlencoding::encode(file_path);
        let url = format!("{}/api/agents/download/{}", self.config.server_url, encoded_path);
        
        let response = self.http_client
            .get(&url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Download failed: {}", response.status()));
        }
        
        // Create parent directory if needed
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Save file
        let mut file = tokio::fs::File::create(dest_path).await?;
        let bytes = response.bytes().await?;
        file.write_all(&bytes).await?;
        
        Ok(())
    }
    
    /// Upload file to server
    pub async fn upload_file(&self, file_path: &std::path::Path, agent_id: Option<&str>, job_id: Option<&str>) -> Result<()> {
        let agent_id_owned = if let Some(aid) = agent_id {
            aid.to_string()
        } else {
            self.agent_id.lock().unwrap().as_deref().map(|s| s.to_string()).unwrap_or_default()
        };
        
        let url = format!("{}/api/agents/upload", self.config.server_url);
        
        let filename = file_path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid file path"))?;
        
        // Read file into memory
        let file_data = tokio::fs::read(file_path).await?;
        
        let mut form = reqwest::multipart::Form::new()
            .text("agent_id", agent_id_owned);
        
        if let Some(jid) = job_id {
            form = form.text("job_id", jid.to_string());
        }
        
        let file_part = reqwest::multipart::Part::bytes(file_data)
            .file_name(filename.to_string())
            .mime_str("application/octet-stream")?;
        
        form = form.part("file", file_part);
        
        let response = self.http_client
            .post(&url)
            .multipart(form)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Upload failed: {}", error_text));
        }
        
        Ok(())
    }
    
    /// Update upscaling job status
    pub async fn update_job_status(&self, job_id: &str, status: &str, progress: Option<f32>, error: Option<&str>) -> Result<()> {
        let url = format!("{}/api/upscaling-jobs/{}/status", self.config.server_url, job_id);
        
        let mut body = serde_json::Map::new();
        body.insert("status".to_string(), serde_json::Value::String(status.to_string()));
        
        if let Some(p) = progress {
            body.insert("progress".to_string(), serde_json::Value::Number(
                serde_json::Number::from_f64(p as f64).unwrap_or_else(|| serde_json::Number::from(0))
            ));
        }
        
        if let Some(e) = error {
            body.insert("error_message".to_string(), serde_json::Value::String(e.to_string()));
        }
        
        let response = self.http_client
            .put(&url)
            .json(&body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to update job status: {}", error_text));
        }
        
        Ok(())
    }
    
    /// Update upscaling job output path
    pub async fn update_job_output(&self, job_id: &str, output_path: &str) -> Result<()> {
        let url = format!("{}/api/upscaling-jobs/{}/output", self.config.server_url, job_id);
        
        let body = serde_json::json!({
            "output_file_path": output_path,
        });
        
        let response = self.http_client
            .put(&url)
            .json(&body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to update job output: {}", error_text));
        }
        
        Ok(())
    }
    
    /// Get agent output location from server
    pub async fn get_output_location(&self) -> Result<Option<String>> {
        let agent_id = self.agent_id.lock().unwrap().clone();
        if let Some(ref agent_id) = agent_id {
            let url = format!("{}/api/agents/{}/output-location", self.config.server_url, agent_id);
            let response = self.http_client
                .get(&url)
                .send()
                .await?;
            
            if response.status().is_success() {
                let result: serde_json::Value = response.json().await?;
                let output_location = result.get("output_location")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Ok(output_location)
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpscalingJob {
    pub id: Option<i64>,
    pub job_id: String,
    pub input_file_path: String,
    pub output_file_path: Option<String>,
    pub show_id: Option<i64>,
    pub topaz_profile_id: Option<i64>,
    pub status: String,
    pub priority: i32,
    pub agent_id: Option<String>,
    pub instruction_id: Option<i64>,
    pub created_at: String,
    pub assigned_at: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub progress: f32,
    pub error_message: Option<String>,
    pub processing_time_seconds: Option<i64>,
}


