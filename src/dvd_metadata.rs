use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

const TMDB_API_BASE: &str = "https://api.themoviedb.org/3";
const TMDB_API_KEY: &str = "fef1285fb85a74350b3292b5fac37fce"; // Users need to set this

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DvdMetadata {
    pub title: String,
    pub year: Option<String>,
    pub media_type: MediaType,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MediaType {
    Movie,
    TVShow,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub season: u32,
    pub episode: u32,
    pub title: String,
    pub title_index: u32, // Which MakeMKV title this corresponds to
}

/// Get DVD disc ID using libdvdread or similar
#[allow(dead_code)]
pub async fn get_dvd_id(device: &str) -> Result<String> {
    use sha1::{Sha1, Digest};
    use base64::Engine;
    
    debug!("Calculating DVD ID for device: {}", device);
    
    // Try to read DVD volume ID
    let output = std::process::Command::new("drutil")
        .arg("status")
        .arg("-drive")
        .arg(device)
        .output()?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Extract volume label if available
    let volume_label = stdout.lines()
        .find(|line| line.contains("Name:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string());
    
    // Use diskutil to get more info
    let diskutil_output = std::process::Command::new("diskutil")
        .arg("info")
        .arg(device)
        .output()?;
    
    let diskutil_stdout = String::from_utf8_lossy(&diskutil_output.stdout);
    
    // Try to extract DVD serial or UUID
    let disc_uuid = diskutil_stdout.lines()
        .find(|line| line.contains("Volume UUID:") || line.contains("Disk UUID:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string());
    
    // Generate a hash from available identifiers
    let mut hasher = Sha1::new();
    
    if let Some(label) = volume_label {
        hasher.update(label.as_bytes());
        debug!("Using volume label for DVD ID");
    }
    
    if let Some(uuid) = disc_uuid {
        hasher.update(uuid.as_bytes());
        debug!("Using UUID for DVD ID");
    }
    
    let hash = hasher.finalize();
    let dvd_id = base64::engine::general_purpose::STANDARD.encode(hash)
        .replace('+', ".")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string();
    
    info!("Calculated DVD ID: {}", dvd_id);
    
    Ok(dvd_id)
}

/// Fetch DVD metadata from TMDB by searching with title
pub async fn fetch_dvd_metadata(_disc_id: &str, volume_name: Option<&str>) -> Result<DvdMetadata> {
    // Check if TMDB API key is configured
    if TMDB_API_KEY == "YOUR_TMDB_API_KEY" {
        warn!("TMDB API key not configured, skipping metadata lookup");
        return create_dummy_dvd_metadata(volume_name);
    }
    
    let client = reqwest::Client::builder()
        .user_agent("Ripley/0.1.0")
        .timeout(Duration::from_secs(10))
        .build()?;
    
    // If we have a volume name, search for it
    if let Some(name) = volume_name {
        info!("Searching TMDB for: {}", name);
        
        // Try TV show first
        if let Ok(metadata) = search_tv_show(&client, name).await {
            return Ok(metadata);
        }
        
        // Fall back to movie search
        if let Ok(metadata) = search_movie(&client, name).await {
            return Ok(metadata);
        }
    }
    
    // If no metadata found, return dummy
    create_dummy_dvd_metadata(volume_name)
}

/// Search TMDB for a TV show
async fn search_tv_show(client: &reqwest::Client, query: &str) -> Result<DvdMetadata> {
    let url = format!(
        "{}/search/tv?api_key={}&query={}",
        TMDB_API_BASE,
        TMDB_API_KEY,
        urlencoding::encode(query)
    );
    
    let response = client.get(&url).send().await?;
    
    if !response.status().is_success() {
        return Err(anyhow!("TMDB search failed: {}", response.status()));
    }
    
    let data: serde_json::Value = response.json().await?;
    
    let results = data["results"].as_array()
        .ok_or_else(|| anyhow!("No results"))?;
    
    if results.is_empty() {
        return Err(anyhow!("No TV shows found"));
    }
    
    // Get the first result
    let show = &results[0];
    let show_id = show["id"].as_i64()
        .ok_or_else(|| anyhow!("No show ID"))?;
    
    let title = show["name"].as_str()
        .unwrap_or("Unknown Show")
        .to_string();
    
    let year = show["first_air_date"].as_str()
        .and_then(|d| d.split('-').next())
        .map(String::from);
    
    info!("Found TV show: {} (ID: {})", title, show_id);
    
    // Get episode details for the first season (most DVDs are single-season)
    let episodes = fetch_tv_episodes(client, show_id, 1).await?;
    
    Ok(DvdMetadata {
        title,
        year,
        media_type: MediaType::TVShow,
        episodes,
    })
}

/// Fetch TV show episodes for a season
async fn fetch_tv_episodes(client: &reqwest::Client, show_id: i64, season: u32) -> Result<Vec<Episode>> {
    let url = format!(
        "{}/tv/{}/season/{}?api_key={}",
        TMDB_API_BASE,
        show_id,
        season,
        TMDB_API_KEY
    );
    
    let response = client.get(&url).send().await?;
    
    if !response.status().is_success() {
        return Ok(Vec::new());
    }
    
    let data: serde_json::Value = response.json().await?;
    
    let episode_list = data["episodes"].as_array()
        .ok_or_else(|| anyhow!("No episodes"))?;
    
    let mut episodes = Vec::new();
    
    for (idx, ep) in episode_list.iter().enumerate() {
        let ep_num = ep["episode_number"].as_u64()
            .unwrap_or((idx + 1) as u64) as u32;
        
        let title = ep["name"].as_str()
            .unwrap_or("Unknown Episode")
            .to_string();
        
        episodes.push(Episode {
            season,
            episode: ep_num,
            title,
            title_index: idx as u32, // Assume titles are in order
        });
    }
    
    info!("Found {} episodes for season {}", episodes.len(), season);
    
    Ok(episodes)
}

/// Search TMDB for a movie
async fn search_movie(client: &reqwest::Client, query: &str) -> Result<DvdMetadata> {
    let url = format!(
        "{}/search/movie?api_key={}&query={}",
        TMDB_API_BASE,
        TMDB_API_KEY,
        urlencoding::encode(query)
    );
    
    let response = client.get(&url).send().await?;
    
    if !response.status().is_success() {
        return Err(anyhow!("TMDB search failed: {}", response.status()));
    }
    
    let data: serde_json::Value = response.json().await?;
    
    let results = data["results"].as_array()
        .ok_or_else(|| anyhow!("No results"))?;
    
    if results.is_empty() {
        return Err(anyhow!("No movies found"));
    }
    
    let movie = &results[0];
    
    let title = movie["title"].as_str()
        .unwrap_or("Unknown Movie")
        .to_string();
    
    let year = movie["release_date"].as_str()
        .and_then(|d| d.split('-').next())
        .map(String::from);
    
    info!("Found movie: {} ({})", title, year.as_deref().unwrap_or("unknown year"));
    
    Ok(DvdMetadata {
        title,
        year,
        media_type: MediaType::Movie,
        episodes: Vec::new(), // Movies don't have episodes
    })
}

/// Create dummy metadata when lookup fails
fn create_dummy_dvd_metadata(volume_name: Option<&str>) -> Result<DvdMetadata> {
    Ok(DvdMetadata {
        title: volume_name.unwrap_or("Unknown DVD").to_string(),
        year: None,
        media_type: MediaType::Unknown,
        episodes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dvd_metadata_structure() {
        let metadata = DvdMetadata {
            title: "Test Show".to_string(),
            year: Some("2025".to_string()),
            media_type: MediaType::TVShow,
            episodes: vec![
                Episode {
                    season: 1,
                    episode: 1,
                    title: "Pilot".to_string(),
                    title_index: 0,
                },
            ],
        };
        
        assert_eq!(metadata.title, "Test Show");
        assert_eq!(metadata.episodes.len(), 1);
        assert_eq!(metadata.media_type, MediaType::TVShow);
    }

    #[test]
    fn test_media_type_variants() {
        assert_eq!(MediaType::Movie, MediaType::Movie);
        assert_eq!(MediaType::TVShow, MediaType::TVShow);
        assert_ne!(MediaType::Movie, MediaType::TVShow);
    }
}
