use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Extract episode title from video using OCR (calls Python toonpipe OCR)
pub async fn extract_episode_title(video_path: &Path) -> Result<Option<String>> {
    info!("Extracting episode title from {} using OCR", video_path.display());
    
    // Check if toonpipe is available
    let check = Command::new("python3")
        .arg("-c")
        .arg("import toonpipe.ocr")
        .output()
        .await;
    
    if check.is_err() || !check.unwrap().status.success() {
        warn!("toonpipe not available for OCR - install with: pip install -e ~/Github/toonpipe");
        return Ok(None);
    }
    
    // Create a simple Python script to call toonpipe OCR with Foster's config
    let python_script = format!(
        r#"
import sys
import yaml
from pathlib import Path
from toonpipe.ocr import find_episode_name

video_path = sys.argv[1]

# Try to load Foster's Home config if it exists
config_path = Path.home() / "Github" / "toonpipe" / "configs" / "fosters-home-for-imaginary-friends.yaml"
config = None
if config_path.exists():
    with open(config_path, 'r') as f:
        config = yaml.safe_load(f)

episode_name, _ = find_episode_name(video_path, config=config)

if episode_name:
    print(episode_name)
else:
    sys.exit(1)
"#
    );
    
    // Run Python script
    let output = Command::new("python3")
        .arg("-c")
        .arg(&python_script)
        .arg(video_path.to_str().unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        debug!("OCR failed: {}", stderr);
        return Ok(None);
    }
    
    let title = String::from_utf8_lossy(&output.stdout).trim().to_string();
    
    if title.is_empty() {
        Ok(None)
    } else {
        info!("OCR extracted title: {}", title);
        Ok(Some(title))
    }
}
