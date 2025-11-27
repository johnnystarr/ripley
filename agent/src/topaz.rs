use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

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
        
        let instance = Self {
            executable_path,
        };
        
        // Validate configuration
        instance.validate_configuration()?;
        
        Ok(instance)
    }
    
    /// Get Topaz version
    pub async fn get_version(&self) -> Result<String> {
        // Try to get version from executable file metadata
        // On Windows, we can check file version info
        #[cfg(target_os = "windows")]
        {
            // Try running Topaz with --version or similar flag
            if let Ok(output) = tokio::process::Command::new(&self.executable_path)
                .arg("--version")
                .output()
                .await
            {
                if let Ok(version_str) = String::from_utf8(output.stdout) {
                    let trimmed = version_str.trim();
                    if !trimmed.is_empty() {
                        return Ok(trimmed.to_string());
                    }
                }
            }
            
            // Try to extract version from path (e.g., "Topaz Video AI 4.0.0" or "Topaz Video AI 5")
            if let Some(file_stem) = self.executable_path.parent() {
                if let Some(parent_name) = file_stem.file_name().and_then(|n| n.to_str()) {
                    // Try to extract version number from folder name
                    let version_re = regex::Regex::new(r"(\d+\.\d+(?:\.\d+)?)").unwrap_or_else(|_| {
                        // Fallback regex if compilation fails
                        regex::Regex::new(r"(\d+)").unwrap()
                    });
                    if let Some(caps) = version_re.captures(parent_name) {
                        if let Some(version) = caps.get(1) {
                            return Ok(version.as_str().to_string());
                        }
                    }
                    return Ok(parent_name.to_string());
                }
            }
            
            Ok("Installed".to_string())
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            Ok("Installed".to_string())
        }
    }
    
    /// Check if Topaz version is supported
    #[allow(dead_code)] // May be used in future version checks
    pub fn is_version_supported(version: &str) -> bool {
        // Extract major version number
        let version_re = regex::Regex::new(r"^(\d+)").unwrap_or_else(|_| {
            regex::Regex::new(r"(\d+)").unwrap()
        });
        if let Some(caps) = version_re.captures(version) {
            if let Some(major_str) = caps.get(1) {
                if let Ok(major) = major_str.as_str().parse::<u32>() {
                    // Support Topaz Video AI 3.x, 4.x, and 5.x
                    return major >= 3 && major <= 5;
                }
            }
        }
        // If we can't parse version, assume it's supported
        true
    }
    
    /// Validate Topaz configuration
    pub fn validate_configuration(&self) -> Result<()> {
        // Check executable exists
        if !self.executable_path.exists() {
            return Err(anyhow::anyhow!("Topaz executable not found at: {:?}", self.executable_path));
        }
        
        // Check executable is actually executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&self.executable_path)?;
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                return Err(anyhow::anyhow!("Topaz executable is not executable"));
            }
        }
        
        Ok(())
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
        
        // Spawn the process and capture output streams
        let mut child = cmd.spawn()?;
        
        // Read stderr in real-time for error detection
        let stderr_handle = if let Some(mut stderr) = child.stderr.take() {
            let _input_path_clone = input_path.to_path_buf();
            let _output_path_clone = output_path.to_path_buf();
            Some(tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(&mut stderr);
                let mut lines = reader.lines();
                
                while let Ok(Some(line)) = lines.next_line().await {
                    let line_lower = line.to_lowercase();
                    
                    // Check for common error patterns
                    if line_lower.contains("error") || line_lower.contains("failed") || line_lower.contains("exception") {
                        error!("Topaz error output: {}", line);
                    } else if line_lower.contains("warning") {
                        warn!("Topaz warning: {}", line);
                    } else {
                        debug!("Topaz stderr: {}", line);
                    }
                }
            }))
        } else {
            None
        };
        
        // Read stdout for progress information
        let stdout_handle = if let Some(mut stdout) = child.stdout.take() {
            Some(tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(&mut stdout);
                let mut lines = reader.lines();
                
                while let Ok(Some(line)) = lines.next_line().await {
                    // Try to parse progress from output
                    // Topaz may output progress in various formats
                    if line.contains("%") || line.contains("progress") || line.contains("frame") {
                        debug!("Topaz progress: {}", line);
                    } else {
                        debug!("Topaz stdout: {}", line);
                    }
                }
            }))
        } else {
            None
        };
        
        // Wait for process to complete
        let status = child.wait().await?;
        
        // Wait for output handlers to finish
        if let Some(handle) = stderr_handle {
            let _ = handle.await;
        }
        if let Some(handle) = stdout_handle {
            let _ = handle.await;
        }
        
        if !status.success() {
            let exit_code = status.code().unwrap_or(-1);
            error!("Topaz upscale failed with exit code: {}", exit_code);
            
            // Provide more helpful error messages based on exit code
            let error_msg = match exit_code {
                1 => "Topaz Video AI: General error or invalid arguments".to_string(),
                2 => "Topaz Video AI: File not found or inaccessible".to_string(),
                3 => "Topaz Video AI: Insufficient resources or memory".to_string(),
                _ => format!("Topaz Video AI failed with exit code: {}", exit_code),
            };
            
            return Err(anyhow::anyhow!("{}", error_msg));
        }
        
        // Verify output file was created
        if !output_path.exists() {
            return Err(anyhow::anyhow!("Topaz upscale completed but output file not found: {:?}", output_path));
        }
        
        // Check output file size (should be non-zero)
        if let Ok(metadata) = std::fs::metadata(&output_path) {
            if metadata.len() == 0 {
                return Err(anyhow::anyhow!("Topaz upscale produced empty output file"));
            }
            info!("Output file size: {} bytes", metadata.len());
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

