use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use crate::agent::{AgentClient, UpscalingJob};
use crate::topaz::{TopazProfile, TopazVideo};

#[derive(Clone)]
pub struct JobWorker {
    agent_client: Arc<AgentClient>,
    topaz: Option<TopazVideo>,
    work_dir: PathBuf,
    current_job: Arc<Mutex<Option<UpscalingJob>>>,
    shutdown: Arc<tokio::sync::Notify>,
    paused: Arc<tokio::sync::RwLock<bool>>,
}

impl JobWorker {
    pub fn new(agent_client: Arc<AgentClient>, work_dir: Option<PathBuf>) -> Result<Self> {
        let work_dir = work_dir.unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".cache").join("ripley-agent").join("work"))
                .unwrap_or_else(|| PathBuf::from("work"))
        });
        
        // Ensure work directory exists
        std::fs::create_dir_all(&work_dir)?;
        
        let topaz = if TopazVideo::is_available() {
            match TopazVideo::new() {
                Ok(t) => {
                    info!("Topaz Video AI initialized");
                    Some(t)
                }
                Err(e) => {
                    warn!("Failed to initialize Topaz: {}", e);
                    None
                }
            }
        } else {
            warn!("Topaz Video AI not found");
            None
        };
        
        Ok(Self {
            agent_client,
            topaz,
            work_dir,
            current_job: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(tokio::sync::Notify::new()),
            paused: Arc::new(tokio::sync::RwLock::new(false)),
        })
    }
    
    /// Start the job worker loop
    pub async fn run(&self) -> Result<()> {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        let shutdown = Arc::clone(&self.shutdown);
        
        loop {
            tokio::select! {
                _ = interval.tick() => {},
                _ = shutdown.notified() => {
                    info!("Job worker shutting down...");
                    break;
                }
            }
            
            // Check if paused
            if *self.paused.read().await {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
            
            // Check if we're already processing a job
            let has_job = self.current_job.lock().await.is_some();
            if has_job {
                continue;
            }
            
            // Try to get next job
            match self.agent_client.get_next_upscaling_job().await {
                Ok(Some(job)) => {
                    info!("Found new upscaling job: {}", job.job_id);
                    
                    // Assign job to this agent
                    if self.agent_client.agent_id().is_some() {
                        // The server should auto-assign when we fetch, but let's set it in our state
                        *self.current_job.lock().await = Some(job.clone());
                        
                        // Process the job asynchronously
                        let agent_client = Arc::clone(&self.agent_client);
                        let topaz = self.topaz.clone();
                        let work_dir = self.work_dir.clone();
                        let current_job = Arc::clone(&self.current_job);
                        
                        tokio::spawn(async move {
                            if let Err(e) = Self::process_job(agent_client, topaz, work_dir, job, current_job).await {
                                error!("Job processing failed: {}", e);
                            }
                        });
                    }
                }
                Ok(None) => {
                    // No jobs available, continue polling
                }
                Err(e) => {
                    warn!("Failed to get next job: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Shutdown the job worker gracefully
    pub async fn shutdown(&self) {
        self.shutdown.notify_one();
    }
    
    /// Get current job (for TUI display)
    pub fn current_job(&self) -> Arc<Mutex<Option<UpscalingJob>>> {
        Arc::clone(&self.current_job)
    }
    
    /// Pause job processing
    pub async fn pause(&self) {
        *self.paused.write().await = true;
        tracing::info!("Job worker paused");
    }
    
    /// Resume job processing
    pub async fn resume(&self) {
        *self.paused.write().await = false;
        tracing::info!("Job worker resumed");
    }
    
    /// Check if paused
    pub async fn is_paused(&self) -> bool {
        *self.paused.read().await
    }
    
    async fn process_job(
        agent_client: Arc<AgentClient>,
        topaz: Option<TopazVideo>,
        _work_dir: PathBuf,
        job: UpscalingJob,
        current_job: Arc<Mutex<Option<UpscalingJob>>>,
    ) -> Result<()> {
        let job_id = job.job_id.clone();
        const MAX_RETRIES: u32 = 3;
        let mut retry_count = 0;
        
        loop {
            info!("Processing job: {} (attempt {}/{})", job_id, retry_count + 1, MAX_RETRIES + 1);
            
            // Update job status to processing
            if let Err(e) = agent_client.update_job_status(&job_id, "processing", Some(0.0), None).await {
                warn!("Failed to update job status: {}", e);
                // Continue anyway - status update failure shouldn't block processing
            }
            
            // Get output location from server (or use default)
            let output_location = agent_client.get_output_location().await?
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .map(|h| h.join("ripley_output"))
                        .unwrap_or_else(|| PathBuf::from("ripley_output"))
                        .to_string_lossy()
                        .to_string()
                });
            
            let output_base = PathBuf::from(&output_location);
            
            // Create folder structure: processing/, upscaled/, encoded/
            let processing_dir = output_base.join("processing");
            let upscaled_dir = output_base.join("upscaled");
            let encoded_dir = output_base.join("encoded");
            
            tokio::fs::create_dir_all(&processing_dir).await?;
            tokio::fs::create_dir_all(&upscaled_dir).await?;
            tokio::fs::create_dir_all(&encoded_dir).await?;
            
            // Download input file to processing folder
            let input_file_name = std::path::Path::new(&job.input_file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid input file path"))?;
            
            let local_input_path = processing_dir.join(&format!("input_{}_{}", job_id, input_file_name));
            
            info!("Downloading input file: {} -> {:?}", job.input_file_path, local_input_path);
            let download_result = agent_client.download_file(&job.input_file_path, &local_input_path).await;
            
            if let Err(e) = download_result {
                error!("Download failed for job {}: {}", job_id, e);
                if retry_count < MAX_RETRIES && Self::is_retryable_error(&e.to_string()) {
                    retry_count += 1;
                    warn!("Retrying download (attempt {}/{})...", retry_count, MAX_RETRIES);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5 * retry_count as u64)).await;
                    continue;
                } else {
                    agent_client.update_job_status(&job_id, "failed", Some(0.0), Some(&format!("Download failed: {}", e))).await?;
                    *current_job.lock().await = None;
                    return Err(e);
                }
            }
            
            // Determine output path in upscaled folder
            let output_file_name = format!("upscaled_{}_{}", job_id, input_file_name);
            let local_output_path = upscaled_dir.join(&output_file_name);
            
            // Load Topaz profile if specified
            let profile: Option<TopazProfile> = if let Some(_profile_id) = job.topaz_profile_id {
                // TODO: Fetch profile from server
                // For now, we'll skip profile loading
                None
            } else {
                None
            };
            
            // Run Topaz upscaling
            if let Some(ref topaz_instance) = topaz {
                info!("Starting Topaz upscale for job: {}", job_id);
                
                // Update progress
                if let Err(e) = agent_client.update_job_status(&job_id, "processing", Some(10.0), None).await {
                    warn!("Failed to update job status: {}", e);
                }
                
                let upscale_result = topaz_instance.upscale(&local_input_path, &local_output_path, profile.as_ref()).await;
                
                match upscale_result {
                    Ok(_) => {
                        info!("Topaz upscale completed for job: {}", job_id);
                        if let Err(e) = agent_client.update_job_status(&job_id, "processing", Some(90.0), None).await {
                            warn!("Failed to update job status: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Topaz upscale failed for job {}: {}", job_id, e);
                        if retry_count < MAX_RETRIES && Self::is_retryable_error(&e.to_string()) {
                            retry_count += 1;
                            warn!("Retrying Topaz upscale (attempt {}/{})...", retry_count, MAX_RETRIES);
                            // Clean up failed output file if it exists
                            let _ = tokio::fs::remove_file(&local_output_path).await;
                            tokio::time::sleep(tokio::time::Duration::from_secs(10 * retry_count as u64)).await;
                            continue;
                        } else {
                            agent_client.update_job_status(&job_id, "failed", Some(0.0), Some(&format!("Topaz error: {}", e))).await?;
                            *current_job.lock().await = None;
                            return Err(e);
                        }
                    }
                }
            } else {
                return Err(anyhow::anyhow!("Topaz Video AI not available"));
            }
            
            // Upload output file
            info!("Uploading output file for job: {}", job_id);
            if let Err(e) = agent_client.update_job_status(&job_id, "processing", Some(95.0), None).await {
                warn!("Failed to update job status: {}", e);
            }
            
            let upload_result = if let Some(agent_id) = agent_client.agent_id() {
                agent_client.upload_file(&local_output_path, Some(&agent_id), Some(&job_id)).await
            } else {
                Err(anyhow::anyhow!("Agent ID not available"))
            };
            
            if let Err(e) = upload_result {
                error!("Upload failed for job {}: {}", job_id, e);
                if retry_count < MAX_RETRIES && Self::is_retryable_error(&e.to_string()) {
                    retry_count += 1;
                    warn!("Retrying upload (attempt {}/{})...", retry_count, MAX_RETRIES);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5 * retry_count as u64)).await;
                    continue;
                } else {
                    agent_client.update_job_status(&job_id, "failed", Some(0.0), Some(&format!("Upload failed: {}", e))).await?;
                    *current_job.lock().await = None;
                    return Err(e);
                }
            }
            
            // Update job output path
            // The server will set this from the upload, but we can also set it explicitly
            let output_path_str = local_output_path.to_string_lossy().to_string();
            if let Err(e) = agent_client.update_job_output(&job_id, &output_path_str).await {
                warn!("Failed to update job output path: {}", e);
            }
            
            // Mark job as completed
            if let Err(e) = agent_client.update_job_status(&job_id, "completed", Some(100.0), None).await {
                warn!("Failed to update job status to completed: {}", e);
            }
            
            info!("Job {} completed successfully", job_id);
            
            // Clean up temporary input file after successful processing
            // Keep upscaled output files for user review
            if let Err(e) = tokio::fs::remove_file(&local_input_path).await {
                warn!("Failed to clean up temporary input file {:?}: {}", local_input_path, e);
            } else {
                info!("Cleaned up temporary input file: {:?}", local_input_path);
            }
            
            // Note: Upscaled files are kept in upscaled/ folder for user review
            // They can be manually cleaned up or managed via a cleanup job
            
            // Clear current job
            *current_job.lock().await = None;
            
            return Ok(());
        }
    }
    
    /// Check if an error is retryable
    fn is_retryable_error(error_msg: &str) -> bool {
        let lower = error_msg.to_lowercase();
        // Network errors, timeouts, and temporary failures are retryable
        lower.contains("timeout") 
            || lower.contains("connection")
            || lower.contains("network")
            || lower.contains("temporary")
            || lower.contains("retry")
            || lower.contains("rate limit")
    }
}

