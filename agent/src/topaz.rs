use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopazProfile {
    pub id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub settings_json: serde_json::Value,
}

#[derive(Clone)]
pub struct TopazVideo {
    executable_path: PathBuf,
}

impl TopazVideo {
    /// Find Topaz Video AI executable
    pub fn find_executable() -> Option<PathBuf> {
        if cfg!(target_os = "windows") {
            let possible_paths = vec![
                r"C:\Program Files\Topaz Labs LLC\Topaz Video AI\Topaz Video AI.exe",
                r"C:\Program Files (x86)\Topaz Labs LLC\Topaz Video AI\Topaz Video AI.exe",
            ];
            
            for path in possible_paths {
                let path_buf = PathBuf::from(path);
                if path_buf.exists() {
                    return Some(path_buf);
                }
            }
        }
        None
    }
    
    /// Create new TopazVideo instance
    pub fn new() -> Result<Self> {
        let executable_path = Self::find_executable()
            .ok_or_else(|| anyhow::anyhow!("Topaz Video AI not found. Please install Topaz Video AI."))?;
        
        info!("Found Topaz Video AI at: {:?}", executable_path);
        
        Ok(Self {
            executable_path,
        })
    }
    
    /// Get Topaz version
    #[allow(dead_code)]
    pub async fn get_version(&self) -> Result<String> {
        // Topaz Video AI doesn't have a --version flag typically
        // We'll try to get version from the executable metadata or use a default
        // For now, return a placeholder
        Ok("Unknown".to_string())
    }
    
    /// Execute upscaling with Topaz Video AI
    pub async fn upscale(
        &self,
        input_path: &Path,
        output_path: &Path,
        profile: Option<&TopazProfile>,
    ) -> Result<()> {
        info!("Starting Topaz upscale: {:?} -> {:?}", input_path, output_path);
        
        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Build command arguments
        // Topaz Video AI CLI typically uses these arguments:
        // -i input file
        // -o output file
        // -p profile (if using profiles)
        
        let mut cmd = Command::new(&self.executable_path);
        cmd.arg("-i").arg(input_path);
        cmd.arg("-o").arg(output_path);
        
        // Apply profile settings if provided
        if let Some(profile) = profile {
            // Topaz profiles might be applied via settings file or command args
            // This depends on Topaz Video AI's actual CLI interface
            // For now, we'll log that a profile is being used
            info!("Using Topaz profile: {}", profile.name);
            
            // If profile has specific settings, apply them
            // The actual implementation depends on Topaz's API
        }
        
        // Add standard arguments
        cmd.arg("--overwrite"); // Allow overwriting output file
        cmd.arg("--quiet"); // Reduce output verbosity
        
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        info!("Executing Topaz command: {:?}", cmd);
        
        let output = cmd.output().await?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Topaz upscale failed: {}", stderr);
            return Err(anyhow::anyhow!("Topaz upscale failed: {}", stderr));
        }
        
        info!("Topaz upscale completed successfully");
        
        Ok(())
    }
    
    /// Check if Topaz is available
    pub fn is_available() -> bool {
        Self::find_executable().is_some()
    }
    
    /// Get executable path
    #[allow(dead_code)]
    pub fn executable_path(&self) -> &Path {
        &self.executable_path
    }
}

