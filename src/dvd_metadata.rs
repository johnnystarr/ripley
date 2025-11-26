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
    pub runtime_minutes: Option<u32>, // Episode runtime in minutes from TMDB
    pub overview: Option<String>, // Episode summary/description from TMDB
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

/// Extract season number from volume name
/// Examples:
/// - "VOLUME 2" -> Some(2)
/// - "SEASON 3" -> Some(3)
/// - "S02" -> Some(2)
/// - "VOL 1" -> Some(1)
#[allow(dead_code)]
pub fn extract_season_from_volume(name: &str) -> Option<u32> {
    let patterns = [
        r"(?i)VOLUME\s*(\d+)",
        r"(?i)VOL\s*(\d+)",
        r"(?i)SEASON\s*(\d+)",
        r"(?i)S(\d+)",
    ];
    
    for pattern in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(name) {
                if let Some(num) = caps.get(1) {
                    if let Ok(season) = num.as_str().parse::<u32>() {
                        return Some(season);
                    }
                }
            }
        }
    }
    
    None
}

/// Clean and normalize a DVD volume name for searching
/// Examples:
/// - "FOSTERS_DISC_ONE" -> "Foster's Home for Imaginary Friends"
/// - "MOVIE_NAME_2023" -> "Movie Name"
/// - "TV_SHOW_S01" -> "TV Show"
fn clean_volume_name(name: &str) -> String {
    let mut cleaned = name.to_string();
    
    // Replace underscores with spaces
    cleaned = cleaned.replace('_', " ");
    
    // Remove common DVD volume patterns
    let patterns_to_remove = [
        r"(?i)\s*DISC\s*\d+",           // DISC 1, DISC_2, etc.
        r"(?i)\s*DISK\s*\d+",           // DISK 1, DISK_2, etc.
        r"(?i)\s*DVD\s*\d*",            // DVD, DVD1, DVD 2, etc.
        r"(?i)\s*CD\s*\d+",             // CD 1, CD_2, etc.
        r"(?i)\s*VOLUME\s*\d+",         // VOLUME 1, etc.
        r"(?i)\s*VOL\s*\d+",            // VOL 1, etc.
        r"(?i)\s*SEASON\s*\d+",         // SEASON 1, etc.
        r"(?i)\s*S\d+",                 // S01, S1, etc.
        r"(?i)\s*\d{4}$",               // Year at end
    ];
    
    for pattern in &patterns_to_remove {
        if let Ok(re) = regex::Regex::new(pattern) {
            cleaned = re.replace_all(&cleaned, "").to_string();
        }
    }
    
    // Trim whitespace
    cleaned = cleaned.trim().to_string();
    
    // Convert to title case (capitalize first letter of each word)
    cleaned = cleaned
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let rest: String = chars.as_str().to_lowercase();
                    format!("{}{}", first.to_uppercase(), rest)
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    
    // Handle possessives: "Fosters" -> "Foster's"
    // This is a heuristic - if a word ends in 's' and is followed by another word,
    // try adding an apostrophe
    cleaned = cleaned.replace("Fosters ", "Foster's ");
    
    cleaned
}

/// Generate multiple search variations from a cleaned volume name
/// This helps when the disc label is abbreviated or formatted oddly
fn generate_search_variations(name: &str) -> Vec<String> {
    let mut variations = Vec::new();
    
    // Always try the cleaned name first
    variations.push(name.to_string());
    
    // Try first word only (e.g., "Fosters One" -> "Fosters")
    if let Some(first_word) = name.split_whitespace().next() {
        if first_word != name {
            variations.push(first_word.to_string());
        }
    }
    
    // Try first two words (e.g., "Fosters One Disc" -> "Fosters One")
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.len() >= 2 {
        variations.push(words[..2].join(" "));
    }
    
    // Try with apostrophe variations
    if name.contains("Fosters") {
        variations.push(name.replace("Fosters", "Foster's"));
    }
    
    // Remove duplicate entries
    variations.sort();
    variations.dedup();
    
    variations
}

/// Fetch DVD metadata from TMDB by searching with title
pub async fn fetch_dvd_metadata(_disc_id: &str, volume_name: Option<&str>) -> Result<DvdMetadata> {
    fetch_dvd_metadata_with_episode(_disc_id, volume_name, None).await
}

/// Fetch DVD metadata from TMDB with optional starting episode hint
pub async fn fetch_dvd_metadata_with_episode(_disc_id: &str, volume_name: Option<&str>, start_episode: Option<u32>) -> Result<DvdMetadata> {
    debug!("fetch_dvd_metadata called with volume_name: {:?}, start_episode: {:?}", volume_name, start_episode);
    
    // Check if TMDB API key is configured
    if TMDB_API_KEY.is_empty() {
        warn!("⚠️  TMDB API key is empty");
        warn!("⚠️  Update TMDB_API_KEY in src/dvd_metadata.rs to enable metadata lookup");
        return create_dummy_dvd_metadata(volume_name);
    }
    
    info!("✅ TMDB API key configured, proceeding with metadata lookup");
    
    let client = reqwest::Client::builder()
        .user_agent("Ripley/0.1.0")
        .timeout(Duration::from_secs(10))
        .build()?;
    
    // If we have a volume name, search for it
    if let Some(name) = volume_name {
        let cleaned_name = clean_volume_name(name);
        info!("📡 Raw volume name: '{}', cleaned: '{}'", name, cleaned_name);
        
        // Generate search variations to try
        let search_terms = generate_search_variations(&cleaned_name);
        info!("🔍 Will try search terms: {:?}", search_terms);
        
        // Try each search variation
        for search_term in &search_terms {
            info!("🔎 Trying TV search: '{}'", search_term);
            if let Ok(metadata) = search_tv_show_with_episode(&client, search_term, start_episode).await {
                info!("✅ Found TV show: {}", metadata.title);
                return Ok(metadata);
            }
            
            info!("🔎 Trying movie search: '{}'", search_term);
            if let Ok(metadata) = search_movie(&client, search_term).await {
                info!("✅ Found movie: {}", metadata.title);
                return Ok(metadata);
            }
        }
        
        warn!("⚠️  No results found on TMDB for any variation of '{}'", name);
    } else {
        warn!("⚠️  No volume name provided, cannot search TMDB");
    }
    
    // If no metadata found, return dummy
    create_dummy_dvd_metadata(volume_name)
}

/// Search TMDB for a TV show (fetches first season only - backward compatible)
#[allow(dead_code)]
async fn search_tv_show(client: &reqwest::Client, query: &str) -> Result<DvdMetadata> {
    search_tv_show_with_episode(client, query, None).await
}

/// Search TMDB for a TV show with optional starting episode hint
async fn search_tv_show_with_episode(client: &reqwest::Client, query: &str, start_episode: Option<u32>) -> Result<DvdMetadata> {
    let url = format!(
        "{}/search/tv?api_key={}&query={}",
        TMDB_API_BASE,
        TMDB_API_KEY,
        urlencoding::encode(query)
    );
    
    debug!("TMDB TV search URL: {}", url.replace(TMDB_API_KEY, "***"));
    
    let response = client.get(&url).send().await?;
    
    if !response.status().is_success() {
        warn!("TMDB API returned error: {}", response.status());
        return Err(anyhow!("TMDB search failed: {}", response.status()));
    }
    
    let data: serde_json::Value = response.json().await?;
    debug!("TMDB TV search response: {}", serde_json::to_string_pretty(&data).unwrap_or_default());
    
    let results = data["results"].as_array()
        .ok_or_else(|| anyhow!("No results"))?;
    
    if results.is_empty() {
        return Err(anyhow!("No TV shows found"));
    }
    
    // Find the best match - prefer titles that contain the search term more prominently
    // For "Fosters", prefer "Foster's Home..." over "The Fosters" since the query is at the start
    let query_lower = query.to_lowercase().replace("'", "");
    let best_match = results.iter()
        .max_by_key(|show| {
            if let Some(name) = show["name"].as_str() {
                let name_lower = name.to_lowercase().replace("'", "");
                
                // Score based on match quality (higher is better)
                let mut score = 0;
                
                // Exact match gets highest score
                if name_lower == query_lower {
                    score += 1000;
                }
                
                // Contains exact query gets high score
                if name_lower.contains(&query_lower) {
                    score += 500;
                }
                
                // All query words present
                if query_lower.split_whitespace().all(|word| name_lower.contains(word)) {
                    score += 100;
                }
                
                // Prefer matches where query appears earlier in title
                if let Some(pos) = name_lower.find(&query_lower) {
                    score += (100 - pos.min(99)) as i32;
                }
                
                // Use popularity as tiebreaker
                if let Some(popularity) = show["popularity"].as_f64() {
                    score += (popularity as i32).min(10);
                }
                
                score
            } else {
                0
            }
        })
        .or_else(|| results.first()); // Fall back to first result if scoring fails
    
    let show = best_match.ok_or_else(|| anyhow!("No show found"))?;
    let show_id = show["id"].as_i64()
        .ok_or_else(|| anyhow!("No show ID"))?;
    
    let title = show["name"].as_str()
        .unwrap_or("Unknown Show")
        .to_string();
    
    let year = show["first_air_date"].as_str()
        .and_then(|d| d.split('-').next())
        .map(String::from);
    
    info!("Found TV show: {} (ID: {})", title, show_id);
    
    // Determine which seasons to fetch based on starting episode
    let seasons_to_fetch = if let Some(start_ep) = start_episode {
        // Estimate which season this episode might be in
        // Assume ~13-26 episodes per season
        let estimated_season = ((start_ep - 1) / 20) + 1;
        info!("Starting episode {} hints at season {}, fetching seasons {}-{}", 
              start_ep, estimated_season, estimated_season.saturating_sub(1).max(1), estimated_season + 1);
        vec![
            estimated_season.saturating_sub(1).max(1), 
            estimated_season,
            estimated_season + 1
        ]
    } else {
        // Default: fetch first 3 seasons to cover most disc sets
        vec![1, 2, 3]
    };
    
    // Fetch episodes from multiple seasons
    let mut all_episodes = Vec::new();
    for season in seasons_to_fetch {
        match fetch_tv_episodes(client, show_id, season).await {
            Ok(mut eps) => {
                info!("✅ Fetched {} episodes from season {}", eps.len(), season);
                all_episodes.append(&mut eps);
            }
            Err(e) => {
                debug!("Could not fetch season {}: {}", season, e);
                // Continue with other seasons
            }
        }
    }
    
    if all_episodes.is_empty() {
        return Err(anyhow!("No episodes found for any season"));
    }
    
    // If we have a starting episode hint, filter to episodes >= that number
    let episodes = if let Some(start_ep) = start_episode {
        info!("Filtering episodes to those >= episode {}", start_ep);
        all_episodes.into_iter()
            .filter(|ep| {
                // Calculate absolute episode number (for multi-season)
                let abs_episode = ep.episode;
                abs_episode >= start_ep || ep.season > 1
            })
            .collect()
    } else {
        all_episodes
    };
    
    info!("Total {} episodes available for matching", episodes.len());
    
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
        
        let runtime_minutes = ep["runtime"].as_u64().map(|r| r as u32);
        
        let overview = ep["overview"].as_str()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        
        episodes.push(Episode {
            season,
            episode: ep_num,
            title,
            title_index: idx as u32, // Will be updated by duration matching
            runtime_minutes,
            overview,
        });
    }
    
    info!("Found {} episodes for season {}", episodes.len(), season);
    
    Ok(episodes)
}

/// Match disc title durations to episodes by runtime
/// Returns updated episodes with correct title_index values
#[allow(unused_mut)]
pub fn match_episodes_by_duration(
    mut episodes: Vec<Episode>,
    title_durations: &[(usize, String)], // (title_index, "HH:MM:SS")
) -> Vec<Episode> {
    info!("Matching {} episodes to {} disc titles by duration", episodes.len(), title_durations.len());
    
    // Convert title durations to minutes
    let mut title_minutes: Vec<(usize, u32)> = Vec::new();
    for (idx, duration_str) in title_durations {
        if let Some(minutes) = parse_duration_to_minutes(duration_str) {
            title_minutes.push((*idx, minutes));
            debug!("Title {}: {} = {} minutes", idx, duration_str, minutes);
        }
    }
    
    // Filter titles that are likely episodes (typically 18-50 minutes for TV)
    let episode_titles: Vec<(usize, u32)> = title_minutes.into_iter()
        .filter(|(_, min)| *min >= 18 && *min <= 50)
        .collect();
    
    info!("Found {} titles that look like TV episodes (18-50 min)", episode_titles.len());
    
    // Match TITLES to EPISODES (not episodes to titles)
    // For each disc title, find the best matching episode by runtime
    let mut matched_episodes = Vec::new();
    let mut used_episodes: std::collections::HashSet<u32> = std::collections::HashSet::new();
    
    for (title_idx, title_min) in &episode_titles {
        // Find best matching episode by runtime (within 5 minutes tolerance)
        let best_match = episodes.iter()
            .filter(|ep| !used_episodes.contains(&ep.episode))
            .filter(|ep| ep.runtime_minutes.is_some())
            .min_by_key(|ep| {
                let ep_runtime = ep.runtime_minutes.unwrap();
                let diff = (*title_min as i32 - ep_runtime as i32).abs();
                diff
            })
            .filter(|ep| {
                let ep_runtime = ep.runtime_minutes.unwrap();
                let diff = (*title_min as i32 - ep_runtime as i32).abs();
                diff <= 5 // Within 5 minutes tolerance
            });
        
        if let Some(episode) = best_match {
            let ep_runtime = episode.runtime_minutes.unwrap();
            info!("Matched Title {} ({} min) to S{}E{:02} '{}' ({} min)", 
                  title_idx, title_min, episode.season, episode.episode, episode.title, ep_runtime);
            
            let mut matched_ep = episode.clone();
            matched_ep.title_index = *title_idx as u32;
            matched_episodes.push(matched_ep);
            used_episodes.insert(episode.episode);
        } else {
            warn!("Could not match Title {} ({} min) to any episode", title_idx, title_min);
        }
    }
    
    // Sort by episode number to maintain order
    matched_episodes.sort_by_key(|ep| ep.episode);
    
    matched_episodes
}

/// Parse duration string "H:MM:SS" or "HH:MM:SS" to minutes
fn parse_duration_to_minutes(duration: &str) -> Option<u32> {
    let parts: Vec<&str> = duration.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    
    let hours: u32 = parts[0].parse().ok()?;
    let minutes: u32 = parts[1].parse().ok()?;
    let seconds: u32 = parts[2].parse().ok()?;
    
    Some(hours * 60 + minutes + if seconds >= 30 { 1 } else { 0 })
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
                    runtime_minutes: Some(22),
                    overview: Some("First episode".to_string()),
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
