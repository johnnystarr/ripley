use anyhow::Result;
use std::path::{Path, PathBuf};
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
        })
    }
    
    /// Start the job worker loop
    pub async fn run(&self) -> Result<()> {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
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
                    if let Some(agent_id) = self.agent_client.agent_id() {
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
    }
    
    async fn process_job(
        agent_client: Arc<AgentClient>,
        topaz: Option<TopazVideo>,
        _work_dir: PathBuf,
        job: UpscalingJob,
        current_job: Arc<Mutex<Option<UpscalingJob>>>,
    ) -> Result<()> {
        let job_id = job.job_id.clone();
        
        info!("Processing job: {}", job_id);
        
        // Update job status to processing
        agent_client.update_job_status(&job_id, "processing", Some(0.0), None).await?;
        
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
        let input_file_name = Path::new(&job.input_file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid input file path"))?;
        
        let local_input_path = processing_dir.join(&format!("input_{}_{}", job_id, input_file_name));
        
        info!("Downloading input file: {} -> {:?}", job.input_file_path, local_input_path);
        agent_client.download_file(&job.input_file_path, &local_input_path).await?;
        
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
            agent_client.update_job_status(&job_id, "processing", Some(10.0), None).await?;
            
            match topaz_instance.upscale(&local_input_path, &local_output_path, profile.as_ref()).await {
                Ok(_) => {
                    info!("Topaz upscale completed for job: {}", job_id);
                    agent_client.update_job_status(&job_id, "processing", Some(90.0), None).await?;
                }
                Err(e) => {
                    error!("Topaz upscale failed for job {}: {}", job_id, e);
                    agent_client.update_job_status(&job_id, "failed", Some(0.0), Some(&format!("Topaz error: {}", e))).await?;
                    *current_job.lock().await = None;
                    return Err(e);
                }
            }
        } else {
            return Err(anyhow::anyhow!("Topaz Video AI not available"));
        }
        
        // Upload output file
        info!("Uploading output file for job: {}", job_id);
        agent_client.update_job_status(&job_id, "processing", Some(95.0), None).await?;
        
        if let Some(agent_id) = agent_client.agent_id() {
            agent_client.upload_file(&local_output_path, Some(&agent_id), Some(&job_id)).await?;
        }
        
        // Update job output path
        // The server will set this from the upload, but we can also set it explicitly
        let output_path_str = local_output_path.to_string_lossy().to_string();
        agent_client.update_job_output(&job_id, &output_path_str).await?;
        
        // Mark job as completed
        agent_client.update_job_status(&job_id, "completed", Some(100.0), None).await?;
        
        info!("Job {} completed successfully", job_id);
        
        // Don't clean up files - keep them in processing/ and upscaled/ folders
        // Files can be manually cleaned up later if needed
        // The processing folder can be cleaned periodically
        
        // Clear current job
        *current_job.lock().await = None;
        
        Ok(())
    }
    
    pub fn current_job(&self) -> Arc<Mutex<Option<UpscalingJob>>> {
        Arc::clone(&self.current_job)
    }
}

