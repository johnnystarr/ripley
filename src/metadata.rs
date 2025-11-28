use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

const MUSICBRAINZ_API: &str = "https://musicbrainz.org/ws/2";
const USER_AGENT: &str = "Ripley/0.1.0 (https://github.com/johnny/ripley)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscMetadata {
    pub artist: String,
    pub album: String,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub number: u32,
    pub title: String,
    pub artist: Option<String>, // For compilations
    pub duration: Option<u32>,  // in seconds
}

/// Fetch metadata for a CD using its disc ID
pub async fn fetch_metadata(disc_id: &str, retry_count: u32) -> Result<DiscMetadata> {
    let mut attempts = 0;
    let max_attempts = retry_count;

    while attempts < max_attempts {
        attempts += 1;
        
        // Try MusicBrainz first
        match fetch_from_musicbrainz(disc_id).await {
            Ok(metadata) => return Ok(metadata),
            Err(e) => {
                warn!("MusicBrainz attempt {}/{} failed: {}", attempts, max_attempts, e);
                
                if attempts < max_attempts {
                    // Try CDDB/freedb as fallback
                    if let Ok(metadata) = fetch_from_cddb(disc_id).await {
                        return Ok(metadata);
                    }
                    
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
    
    Err(anyhow!("Failed to fetch metadata after {} attempts", max_attempts))
}

/// Fetch metadata from MusicBrainz API
async fn fetch_from_musicbrainz(disc_id: &str) -> Result<DiscMetadata> {
    let url = format!("{}/discid/{}", MUSICBRAINZ_API, disc_id);
    debug!("Fetching metadata from MusicBrainz: {}", url);
    
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;
    
    let response = client
        .get(&url)
        .query(&[("inc", "artists+recordings+release-groups")])
        .send()
        .await
        .context("Failed to fetch from MusicBrainz")?;
    
    if !response.status().is_success() {
        return Err(anyhow!("MusicBrainz API returned status: {}", response.status()));
    }
    
    let json: serde_json::Value = response.json().await?;
    
    // Parse MusicBrainz response
    let release = json["releases"]
        .as_array()
        .and_then(|releases| releases.first())
        .ok_or_else(|| anyhow!("No releases found"))?;
    
    let release_group = release["release-group"]
        .as_object()
        .ok_or_else(|| anyhow!("No release group found"))?;
    
    let artist_credits = release["artist-credit"]
        .as_array()
        .ok_or_else(|| anyhow!("No artist credits found"))?;
    
    let artist = artist_credits
        .iter()
        .filter_map(|credit| credit["artist"]["name"].as_str())
        .collect::<Vec<_>>()
        .join(", ");
    
    let album = release_group["title"]
        .as_str()
        .unwrap_or("Unknown Album")
        .to_string();
    
    let year = release_group["first-release-date"]
        .as_str()
        .and_then(|date| date.split('-').next())
        .map(String::from);
    
    let genre = release_group["primary-type"]
        .as_str()
        .map(String::from);
    
    let medium = release["media"]
        .as_array()
        .and_then(|media| media.first())
        .ok_or_else(|| anyhow!("No medium found"))?;
    
    let tracks_data = medium["tracks"]
        .as_array()
        .ok_or_else(|| anyhow!("No tracks found"))?;
    
    let mut tracks = Vec::new();
    for track_data in tracks_data {
        let recording = track_data["recording"]
            .as_object()
            .ok_or_else(|| anyhow!("No recording data"))?;
        
        let number = track_data["number"]
            .as_str()
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(0);
        
        let title = recording["title"]
            .as_str()
            .unwrap_or("Unknown Track")
            .to_string();
        
        let duration = recording["length"]
            .as_u64()
            .map(|ms| (ms / 1000) as u32);
        
        tracks.push(Track {
            number,
            title,
            artist: None, // MusicBrainz doesn't provide per-track artists in this structure
            duration,
        });
    }
    
    if tracks.is_empty() {
        return Err(anyhow!("No tracks found in release"));
    }
    
    info!("Found metadata: {} - {} ({} tracks)", artist, album, tracks.len());
    
    Ok(DiscMetadata {
        artist: if artist.is_empty() { "Unknown Artist".to_string() } else { artist },
        album,
        year,
        genre,
        tracks,
    })
}

/// Fetch metadata from CDDB/freedb (fallback)
async fn fetch_from_cddb(_disc_id: &str) -> Result<DiscMetadata> {
    // CDDB lookup would go here
    // For now, just return an error to indicate it's not implemented
    Err(anyhow!("CDDB lookup not implemented"))
}

/// Calculate MusicBrainz disc ID from CD TOC (pure Rust implementation)
pub async fn get_disc_id(device: &str) -> Result<String> {
    use sha1::{Sha1, Digest};
    use base64::Engine;
    
    debug!("Calculating disc ID for device: {}", device);
    
    // Get TOC from cd-discid
    let output = std::process::Command::new("cd-discid")
        .arg(device)
        .output()
        .context("Failed to run cd-discid")?;
    
    if !output.status.success() {
        return Err(anyhow!("cd-discid failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    
    let toc = String::from_utf8_lossy(&output.stdout);
    debug!("cd-discid output: {}", toc);
    
    // Parse: discid numtracks offset1 offset2 ... offsetN length
    let parts: Vec<&str> = toc.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(anyhow!("Invalid cd-discid output"));
    }
    
    let num_tracks: u32 = parts[1].parse()
        .context("Failed to parse track count")?;
    
    // Validate track count (audio CDs support 1-99 tracks)
    if num_tracks == 0 || num_tracks > 99 {
        return Err(anyhow!("Invalid track count: {}", num_tracks));
    }
    
    // Get offsets (first track offset is typically 150 frames = 2 seconds)
    if parts.len() < (num_tracks as usize + 2) {
        return Err(anyhow!("Insufficient cd-discid data: got {} parts, expected at least {}", 
                          parts.len(), num_tracks + 2));
    }
    
    let offsets: Vec<u32> = parts[2..(num_tracks as usize + 2)]
        .iter()
        .map(|s| s.parse())
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse track offsets")?;
    
    // Get exact leadout offset from drutil (cd-discid rounds seconds, losing precision)
    // For now, use cd-discid's length value
    let leadout: u32 = if parts.len() > (num_tracks as usize + 2) {
        parts[num_tracks as usize + 2].parse()
            .unwrap_or(offsets.last().copied().unwrap_or(0) + 10000)
    } else {
        offsets.last().copied().unwrap_or(0) + 10000 // Fallback
    };
    
    // Calculate MusicBrainz disc ID
    // Format: SHA1 hash of "numtracks offset1 offset2 ... offsetN leadout"
    let mut hasher = Sha1::new();
    hasher.update(num_tracks.to_string());
    hasher.update(" ");
    for (i, offset) in offsets.iter().enumerate() {
        if i > 0 {
            hasher.update(" ");
        }
        hasher.update(offset.to_string());
    }
    hasher.update(" ");
    hasher.update(leadout.to_string());
    
    let hash = hasher.finalize();
    let disc_id = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);
    
    debug!("Calculated disc ID: {}", disc_id);
    Ok(disc_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disc_metadata_structure() {
        let metadata = DiscMetadata {
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            year: Some("2025".to_string()),
            genre: Some("Rock".to_string()),
            tracks: vec![
                Track {
                    number: 1,
                    title: "Track One".to_string(),
                    artist: None,
                    duration: Some(180),
                },
                Track {
                    number: 2,
                    title: "Track Two".to_string(),
                    artist: Some("Featured Artist".to_string()),
                    duration: Some(240),
                },
            ],
        };

        assert_eq!(metadata.tracks.len(), 2);
        assert_eq!(metadata.artist, "Test Artist");
        assert_eq!(metadata.album, "Test Album");
        assert!(metadata.year.is_some());
        assert_eq!(metadata.tracks[0].number, 1);
        assert_eq!(metadata.tracks[1].artist, Some("Featured Artist".to_string()));
    }

    #[test]
    fn test_track_structure() {
        let track = Track {
            number: 5,
            title: "Test Track".to_string(),
            artist: Some("Artist Name".to_string()),
            duration: Some(300),
        };

        assert_eq!(track.number, 5);
        assert_eq!(track.title, "Test Track");
        assert_eq!(track.artist, Some("Artist Name".to_string()));
        assert_eq!(track.duration, Some(300));
    }

    #[test]
    fn test_disc_metadata_serialization() {
        let metadata = DiscMetadata {
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            year: Some("2025".to_string()),
            genre: None,
            tracks: vec![],
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("Artist"));
        assert!(json.contains("Album"));
        assert!(json.contains("2025"));
    }

    #[test]
    fn test_disc_metadata_with_no_year() {
        let metadata = DiscMetadata {
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            year: None,
            genre: None,
            tracks: vec![],
        };

        assert!(metadata.year.is_none());
        assert_eq!(metadata.artist, "Artist");
    }

    #[test]
    fn test_multiple_tracks() {
        let metadata = DiscMetadata {
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            year: None,
            genre: None,
            tracks: (1..=10)
                .map(|n| Track {
                    number: n,
                    title: format!("Track {}", n),
                    artist: None,
                    duration: Some(180),
                })
                .collect(),
        };

        assert_eq!(metadata.tracks.len(), 10);
        assert_eq!(metadata.tracks[0].number, 1);
        assert_eq!(metadata.tracks[9].number, 10);
    }

    #[test]
    fn test_track_with_featured_artist() {
        let track = Track {
            number: 1,
            title: "Song Title".to_string(),
            artist: Some("Feat. Other Artist".to_string()),
            duration: Some(200),
        };

        assert_eq!(track.artist, Some("Feat. Other Artist".to_string()));
    }
}
