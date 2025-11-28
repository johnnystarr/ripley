use anyhow::Result;
use tracing::{debug, warn};

const NTFY_TOPIC: &str = "staryavsky_alerts";
const NTFY_URL: &str = "https://ntfy.sh";

pub enum DiscType {
    CD,
    #[allow(clippy::upper_case_acronyms)]
    DVD,
    BluRay,
}

impl DiscType {
    fn emoji(&self) -> &str {
        match self {
            DiscType::CD => "💿",
            DiscType::DVD => "📀",
            DiscType::BluRay => "📀",
        }
    }
    
    fn name(&self) -> &str {
        match self {
            DiscType::CD => "CD",
            DiscType::DVD => "DVD",
            DiscType::BluRay => "Blu-ray",
        }
    }
}

pub struct DiscInfo {
    pub disc_type: DiscType,
    pub title: String,
    pub device: String,
}

/// Send a notification to ntfy.sh when disc ripping completes
pub async fn send_completion_notification(disc_info: DiscInfo, success: bool) -> Result<()> {
    let emoji = if success { "✅" } else { "❌" };
    let status = if success { "Complete" } else { "Failed" };
    let priority = if success { "default" } else { "high" };
    
    let title_text = format!(
        "{} {} {} Rip {}",
        emoji,
        disc_info.disc_type.emoji(),
        disc_info.disc_type.name(),
        status
    );
    
    let message = format!(
        "{}\nDevice: {}",
        disc_info.title,
        disc_info.device
    );
    
    debug!("Sending ntfy notification: {} - {}", title_text, message);
    
    let client = reqwest::Client::new();
    let url = format!("{}/{}", NTFY_URL, NTFY_TOPIC);
    
    let response = client
        .post(&url)
        .header("Title", title_text)
        .header("Priority", priority)
        .header("Tags", if success { 
            format!("{},white_check_mark", disc_info.disc_type.name().to_lowercase())
        } else {
            format!("{},x", disc_info.disc_type.name().to_lowercase())
        })
        .body(message)
        .send()
        .await;
    
    match response {
        Ok(resp) if resp.status().is_success() => {
            debug!("Notification sent successfully");
            Ok(())
        }
        Ok(resp) => {
            warn!("Notification failed with status: {}", resp.status());
            Err(anyhow::anyhow!("Notification failed: {}", resp.status()))
        }
        Err(e) => {
            warn!("Failed to send notification: {}", e);
            Err(anyhow::anyhow!("Failed to send notification: {}", e))
        }
    }
}
