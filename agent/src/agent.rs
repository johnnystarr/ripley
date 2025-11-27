use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
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
    
    /// Download file from server with progress tracking and resume support
    pub async fn download_file(&self, file_path: &str, dest_path: &std::path::Path) -> Result<()> {
        use tokio::io::{AsyncSeekExt, AsyncWriteExt};
        use sha2::{Sha256, Digest};
        
        // Check disk space before download (basic check)
        // Full implementation would use platform-specific APIs
        // For now, we'll check file size from headers after request
        
        // URL encode the file path
        let encoded_path = urlencoding::encode(file_path);
        let url = format!("{}/api/agents/download/{}", self.config.server_url, encoded_path);
        
        // Check if file exists for resume
        let mut resume_from = 0u64;
        let mut file = if dest_path.exists() {
            // Try to resume from existing file
            let metadata = tokio::fs::metadata(dest_path).await?;
            resume_from = metadata.len();
            tracing::info!("Resuming download from byte {}", resume_from);
            tokio::fs::OpenOptions::new()
                .write(true)
                .open(dest_path).await?
        } else {
            // Create parent directory if needed
            if let Some(parent) = dest_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::File::create(dest_path).await?
        };
        
        // Seek to resume position if resuming
        if resume_from > 0 {
            file.seek(tokio::io::SeekFrom::Start(resume_from)).await?;
        }
        
        let mut request = self.http_client.get(&url);
        
        // Add Range header for resume
        if resume_from > 0 {
            request = request.header("Range", format!("bytes={}-", resume_from));
        }
        
        let response = request.send().await?;
        
        // Handle partial content (206) for resume, or regular (200) for new download
        let status_code = response.status().as_u16();
        if !response.status().is_success() && status_code != 206 {
            return Err(anyhow::anyhow!("Download failed: {}", response.status()));
        }
        
        // Get expected checksum and file size from headers
        let expected_checksum = response.headers()
            .get("X-File-Checksum")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());
        
        let total_size = response.headers()
            .get("Content-Length")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| {
                response.headers()
                    .get("Content-Range")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| {
                        // Parse "bytes start-end/total" format
                        s.split('/').nth(1)?.parse::<u64>().ok()
                    })
            });
        
        // Download in chunks with progress tracking
        // Note: Disk space checking is handled by the OS - download will fail if insufficient space
        let mut hasher = Sha256::new();
        let mut stream = response.bytes_stream();
        let mut downloaded = resume_from;
        
        use futures::StreamExt;
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            
            // Log progress periodically (every 10MB)
            if downloaded % (10 * 1024 * 1024) < chunk.len() as u64 {
                if let Some(total) = total_size {
                    let percent = (downloaded as f64 / total as f64) * 100.0;
                    tracing::debug!("Download progress: {:.1}% ({}/{} bytes)", percent, downloaded, total);
                } else {
                    tracing::debug!("Downloaded: {} bytes", downloaded);
                }
            }
        }
        
        file.sync_all().await?;
        
        // Verify checksum if provided (only for complete downloads)
        if resume_from == 0 {
            if let Some(expected) = &expected_checksum {
                let calculated = format!("{:x}", hasher.finalize());
                if calculated != *expected {
                    // Clean up corrupted file
                    let _ = tokio::fs::remove_file(dest_path).await;
                    return Err(anyhow::anyhow!("Checksum mismatch: expected {}, got {}", expected, calculated));
                }
                tracing::info!("File checksum verified: {}", calculated);
            }
        }
        
        Ok(())
    }
    
    /// Upload file to server with progress tracking and retry logic
    pub async fn upload_file(&self, file_path: &std::path::Path, agent_id: Option<&str>, job_id: Option<&str>) -> Result<()> {
        const MAX_RETRIES: u32 = 3;
        let mut retry_count = 0;
        
        loop {
            let agent_id_owned = if let Some(aid) = agent_id {
                aid.to_string()
            } else {
                self.agent_id.lock().unwrap().as_deref().map(|s| s.to_string()).unwrap_or_default()
            };
            
            let url = format!("{}/api/agents/upload", self.config.server_url);
            
            let filename = file_path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid file path"))?;
            
            // Get file size for progress tracking
            let file_metadata = tokio::fs::metadata(file_path).await?;
            let file_size = file_metadata.len();
            
            // For large files, read in chunks to avoid loading entire file into memory
            // For now, we'll read the whole file but this can be optimized for very large files
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
            
            tracing::info!("Uploading file: {} ({} bytes)", filename, file_size);
            
            let response_result = self.http_client
                .post(&url)
                .multipart(form)
                .send()
                .await;
            
            match response_result {
                Ok(response) => {
                    if response.status().is_success() {
                        tracing::info!("Upload completed successfully: {} bytes", file_size);
                        return Ok(());
                    } else {
                        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                        if retry_count < MAX_RETRIES {
                            retry_count += 1;
                            tracing::warn!("Upload failed (attempt {}/{}): {}, retrying...", retry_count, MAX_RETRIES, error_text);
                            tokio::time::sleep(tokio::time::Duration::from_secs(2 * retry_count as u64)).await;
                            continue;
                        }
                        return Err(anyhow::anyhow!("Upload failed after {} retries: {}", MAX_RETRIES, error_text));
                    }
                }
                Err(e) => {
                    if retry_count < MAX_RETRIES && Self::is_retryable_upload_error(&e.to_string()) {
                        retry_count += 1;
                        tracing::warn!("Upload error (attempt {}/{}): {}, retrying...", retry_count, MAX_RETRIES, e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(2 * retry_count as u64)).await;
                        continue;
                    }
                    return Err(anyhow::anyhow!("Upload failed: {}", e));
                }
            }
        }
    }
    
    /// Check if upload error is retryable
    fn is_retryable_upload_error(error_msg: &str) -> bool {
        let lower = error_msg.to_lowercase();
        lower.contains("timeout") 
            || lower.contains("connection")
            || lower.contains("network")
            || lower.contains("temporary")
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
    
    /// Get Topaz profile by ID from server
    pub async fn get_topaz_profile(&self, profile_id: i64) -> Result<Option<crate::topaz::TopazProfile>> {
        let url = format!("{}/api/topaz-profiles/{}", self.config.server_url, profile_id);
        let response = self.http_client
            .get(&url)
            .send()
            .await?;
        
        if response.status().is_success() {
            let profile: crate::topaz::TopazProfile = response.json().await?;
            Ok(Some(profile))
        } else if response.status().as_u16() == 404 {
            Ok(None)
        } else {
            let error_text = response.text().await?;
            Err(anyhow::anyhow!("Failed to get Topaz profile: {}", error_text))
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


