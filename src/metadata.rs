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

/// Fetch metadata from MusicBrainz
async fn fetch_from_musicbrainz(disc_id: &str) -> Result<DiscMetadata> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(10))
        .build()?;

    // First, look up the disc ID
    let url = format!("{}/discid/{}?fmt=json&inc=recordings+artist-credits", MUSICBRAINZ_API, disc_id);
    
    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to query MusicBrainz")?;

    if !response.status().is_success() {
        return Err(anyhow!("MusicBrainz returned status: {}", response.status()));
    }

    let data: serde_json::Value = response.json().await?;
    
    // Parse the response
    parse_musicbrainz_response(data)
}

/// Parse MusicBrainz JSON response
fn parse_musicbrainz_response(data: serde_json::Value) -> Result<DiscMetadata> {
    let releases = data["releases"].as_array()
        .ok_or_else(|| anyhow!("No releases found"))?;
    
    if releases.is_empty() {
        return Err(anyhow!("No releases in response"));
    }

    // Get the first release
    let release = &releases[0];
    
    let artist = release["artist-credit"][0]["artist"]["name"]
        .as_str()
        .unwrap_or("Unknown Artist")
        .to_string();
    
    let album = release["title"]
        .as_str()
        .unwrap_or("Unknown Album")
        .to_string();
    
    let year = release["date"]
        .as_str()
        .and_then(|d| d.split('-').next())
        .map(String::from);

    // Parse tracks
    let mut tracks = Vec::new();
    if let Some(media) = release["media"].as_array() {
        if let Some(first_medium) = media.first() {
            if let Some(track_list) = first_medium["tracks"].as_array() {
                for (idx, track) in track_list.iter().enumerate() {
                    let title = track["recording"]["title"]
                        .as_str()
                        .unwrap_or("Unknown Track")
                        .to_string();
                    
                    let duration = track["length"]
                        .as_u64()
                        .map(|ms| (ms / 1000) as u32);

                    tracks.push(Track {
                        number: (idx + 1) as u32,
                        title,
                        artist: None,
                        duration,
                    });
                }
            }
        }
    }

    Ok(DiscMetadata {
        artist,
        album,
        year,
        genre: None,
        tracks,
    })
}

/// Fetch metadata from CDDB/FreeDB (fallback)
async fn fetch_from_cddb(disc_id: &str) -> Result<DiscMetadata> {
    // Note: FreeDB has been shut down, but gnudb.org is a mirror
    // This is a simplified implementation - in production you'd want to use the full CDDB protocol
    
    debug!("Attempting CDDB lookup for {}", disc_id);
    
    // For now, return an error as CDDB requires more complex protocol implementation
    Err(anyhow!("CDDB lookup not yet implemented"))
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
    
    let mut offsets: Vec<u32> = Vec::new();
    for i in 2..(2 + num_tracks as usize) {
        offsets.push(parts[i].parse()
            .context("Failed to parse offset")?);
    }
    
    // Get exact leadout offset from drutil (cd-discid rounds seconds, losing precision)
    // Leadout = disc blocks + first track offset
    let drutil_output = std::process::Command::new("drutil")
        .arg("status")
        .output()
        .context("Failed to run drutil")?;
    
    let drutil_text = String::from_utf8_lossy(&drutil_output.stdout);
    let blocks_line = drutil_text.lines()
        .find(|line| line.contains("Space Used:") && line.contains("blocks:"))
        .context("Could not find blocks in drutil output")?;
    
    // Parse "Space Used:   42:54:05         blocks:   193055 / ..."
    let blocks_str = blocks_line.split("blocks:")
        .nth(1)
        .and_then(|s| s.trim().split_whitespace().next())
        .context("Could not parse blocks from drutil")?;
    
    let disc_blocks: u32 = blocks_str.parse()
        .context("Failed to parse block count")?;
    
    let first_track_offset = offsets.first().copied().unwrap_or(150);
    let leadout_offset = disc_blocks + first_track_offset;
    
    debug!("Disc blocks: {}, first track offset: {}, leadout: {}", 
           disc_blocks, first_track_offset, leadout_offset);
    
    // Calculate MusicBrainz disc ID using SHA-1
    // Algorithm from: https://musicbrainz.org/doc/Disc_ID_Calculation
    let mut hasher = Sha1::new();
    
    let first_track = 1u8;  // Audio CDs always start at track 1
    let last_track = num_tracks as u8;
    
    // First track number (1 byte as 2 hex chars)
    hasher.update(format!("{:02X}", first_track).as_bytes());
    
    // Last track number (1 byte as 2 hex chars)
    hasher.update(format!("{:02X}", last_track).as_bytes());
    
    // Lead-out track offset (4 bytes as 8 hex chars) - this is FrameOffset[0]
    hasher.update(format!("{:08X}", leadout_offset).as_bytes());
    
    // 99 frame offsets (4 bytes as 8 hex chars each) - FrameOffset[1..99]
    // Position i in the array corresponds to track i
    // So track 1 goes in position 1, track 2 in position 2, etc.
    for i in 1..=99 {
        let offset = if i >= first_track as usize && i <= last_track as usize {
            // This position has a valid track
            let track_index = i - first_track as usize;
            offsets.get(track_index).copied().unwrap_or(0)
        } else {
            // No track at this position
            0
        };
        hasher.update(format!("{:08X}", offset).as_bytes());
    }
    
    // Compute SHA-1 hash
    let hash = hasher.finalize();
    
    // Encode as base64 with MusicBrainz special characters
    // MusicBrainz uses: + -> ., / -> _, = -> -
    let disc_id = base64::engine::general_purpose::STANDARD.encode(&hash)
        .replace('+', ".")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string() + "-";
    
    info!("Calculated MusicBrainz disc ID: {}", disc_id);
    
    Ok(disc_id)
}
