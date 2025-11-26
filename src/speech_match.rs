use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct EpisodeMatch {
    pub season: u32,
    pub episode: u32,
    pub title: String,
    pub confidence: f32,
}

/// Extract subtitles from video file or transcribe audio as fallback
pub async fn extract_and_transcribe_audio(video_path: &Path) -> Result<String> {
    // First, try to extract embedded subtitles using ffmpeg
    if let Ok(subtitles) = extract_subtitles_from_video(video_path).await {
        info!("Extracted embedded subtitles: {} characters", subtitles.len());
        return Ok(subtitles);
    }
    
    info!("No embedded subtitles found, falling back to audio transcription");
    extract_and_transcribe_audio_segments(video_path).await
}

/// Extract embedded subtitles from video file (SRT, ASS, SSA, etc.)
async fn extract_subtitles_from_video(video_path: &Path) -> Result<String> {
    let subtitle_file = "/tmp/ripley_subtitles.srt";
    
    // Try to extract subtitles using ffmpeg
    let extract_result = Command::new("ffmpeg")
        .args(&[
            "-i", video_path.to_str().unwrap(),
            "-map", "0:s:0",  // First subtitle stream
            "-f", "srt",  // Force SRT format
            "-y",  // Overwrite
            subtitle_file
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;
    
    if !extract_result.success() {
        return Err(anyhow::anyhow!("No subtitles found in video"));
    }
    
    // Read subtitle file and extract dialogue only (strip timestamps and formatting)
    let subtitle_content = tokio::fs::read_to_string(subtitle_file).await?;
    let dialogue = parse_srt_dialogue(&subtitle_content);
    
    // Clean up
    let _ = tokio::fs::remove_file(subtitle_file).await;
    
    if dialogue.is_empty() {
        return Err(anyhow::anyhow!("Subtitle file was empty"));
    }
    
    Ok(dialogue)
}

/// Parse SRT content and extract just the dialogue lines
fn parse_srt_dialogue(srt_content: &str) -> String {
    let mut dialogue_lines = Vec::new();
    
    for line in srt_content.lines() {
        let trimmed = line.trim();
        
        // Skip empty lines, numbers, and timestamp lines
        if trimmed.is_empty() 
            || trimmed.chars().all(|c| c.is_numeric())
            || trimmed.contains("-->") {
            continue;
        }
        
        // Skip HTML/formatting tags
        let cleaned = trimmed
            .replace("<i>", "")
            .replace("</i>", "")
            .replace("<b>", "")
            .replace("</b>", "")
            .replace("<u>", "")
            .replace("</u>", "");
        
        if !cleaned.is_empty() {
            dialogue_lines.push(cleaned);
        }
    }
    
    dialogue_lines.join(" ")
}

/// Extract audio from multiple segments (beginning, middle, end) and transcribe
async fn extract_and_transcribe_audio_segments(video_path: &Path) -> Result<String> {
    info!("Extracting audio from multiple segments of {}", video_path.display());
    
    // Get video duration
    let duration_output = Command::new("ffprobe")
        .args(&[
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            video_path.to_str().unwrap()
        ])
        .output()
        .await?;
    
    let duration_str = String::from_utf8_lossy(&duration_output.stdout);
    let duration: f64 = duration_str.trim().parse()?;
    
    // Extract 3 segments: right after intro, middle, and before credits
    // Intros are usually 30-60 seconds, credits at the very end
    let segment_duration = 90.0; // 1.5 minutes per segment for more context
    let segments = vec![
        (90.0, "opening"),  // Right after intro - usually has unique episode setup
        (duration * 0.40, "early-middle"),  // Earlier middle for more plot
        (duration * 0.65, "late-middle"),  // Later middle for climax
    ];
    
    info!("Video duration: {:.0}s, extracting 3 segments of {}s each", duration, segment_duration);
    
    let mut all_transcripts = Vec::new();
    
    for (i, (start_time, label)) in segments.iter().enumerate() {
        let audio_file = format!("/tmp/ripley_audio_segment_{}.wav", i);
        
        // Extract segment
        let extract_result = Command::new("ffmpeg")
            .args(&[
                "-ss", &start_time.to_string(),
                "-i", video_path.to_str().unwrap(),
                "-t", &segment_duration.to_string(),
                "-vn",  // No video
                "-acodec", "pcm_s16le",  // PCM WAV format
                "-ar", "16000",  // 16kHz sample rate (good for speech)
                "-ac", "1",  // Mono
                "-y",  // Overwrite
                &audio_file
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        
        if !extract_result.success() {
            warn!("Failed to extract {} segment, continuing...", label);
            continue;
        }
        
        // Transcribe segment
        let transcript = transcribe_with_openai_api(&audio_file).await?;
        info!("Transcribed {} segment: {} characters", label, transcript.len());
        all_transcripts.push(format!("[{}]: {}", label.to_uppercase(), transcript));
        
        // Clean up temp file
        let _ = tokio::fs::remove_file(&audio_file).await;
    }
    
    if all_transcripts.is_empty() {
        return Err(anyhow::anyhow!("Failed to transcribe any segments"));
    }
    
    // Combine all transcripts
    let combined = all_transcripts.join("\n\n");
    info!("Combined transcription complete: {} characters total", combined.len());
    Ok(combined)
}

/// Transcribe audio using OpenAI Whisper API
async fn transcribe_with_openai_api(audio_path: &str) -> Result<String> {
    // Load config and get API key
    let config = crate::config::Config::load()?;
    let api_key = config.get_openai_api_key()
        .ok_or_else(|| anyhow::anyhow!("OpenAI API key not configured in config.yaml or OPENAI_API_KEY env var"))?;
    
    info!("Using OpenAI Whisper API for transcription");
    
    // Use curl to upload audio file to OpenAI Whisper API
    let output = Command::new("curl")
        .args(&[
            "-X", "POST",
            "https://api.openai.com/v1/audio/transcriptions",
            "-H", &format!("Authorization: Bearer {}", api_key),
            "-F", &format!("file=@{}", audio_path),
            "-F", "model=whisper-1"
        ])
        .output()
        .await?;
    
    if !output.status.success() {
        return Err(anyhow::anyhow!("OpenAI API request failed"));
    }
    
    let response = String::from_utf8_lossy(&output.stdout);
    
    // Parse JSON response
    let json: serde_json::Value = serde_json::from_str(&response)?;
    let transcript = json["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No transcript in response"))?
        .to_string();
    
    Ok(transcript)
}

/// Match transcript against TMDB episodes using OpenAI
pub async fn match_episode_by_transcript(
    show_name: &str,
    transcript: &str,
    episodes: &[crate::dvd_metadata::Episode],
) -> Result<EpisodeMatch> {
    info!("Matching transcript against {} episodes", episodes.len());
    
    // Load config and get API key
    let config = crate::config::Config::load()?;
    let api_key = config.get_openai_api_key()
        .ok_or_else(|| anyhow::anyhow!("OpenAI API key not configured in config.yaml or OPENAI_API_KEY env var"))?;
    
    // Build episode list with summaries for better context
    let episode_list: Vec<String> = episodes.iter()
        .map(|ep| {
            if let Some(overview) = &ep.overview {
                format!("S{:02}E{:02}: {} - {}", ep.season, ep.episode, ep.title, overview)
            } else {
                format!("S{:02}E{:02}: {}", ep.season, ep.episode, ep.title)
            }
        })
        .collect();
    
    let prompt = format!(
        r#"You are matching a TV episode transcript to the correct episode.

Show: {}

Available episodes (with plot summaries):
{}

Dialogue transcript from the episode:
{}

Task: Match this dialogue to the correct episode by:
1. Identifying key plot points, character interactions, and story elements from the dialogue
2. Comparing these elements to each episode's plot summary
3. Finding the episode whose summary best matches the events/dialogue shown

Respond with episode code and confidence (0-100).
Format: S##E## <confidence>
Example: S01E13 95"#,
        show_name,
        episode_list.join("\n"),
        transcript.chars().take(3000).collect::<String>()
    );
    
    debug!("Sending to OpenAI: {} chars", prompt.len());
    
    // Call OpenAI API
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "You are a TV episode identification assistant with expertise in analyzing dialogue and matching it to episode summaries. Respond with only the episode code and confidence."},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.2
        }))
        .send()
        .await?;
    
    let result: serde_json::Value = response.json().await?;
    let answer = result["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No response from OpenAI"))?;
    
    debug!("OpenAI response: {}", answer);
    
    // Parse response like "S02E05" or "S02E05\n85" or "S02E05 confidence: 85"
    // Use regex to extract season/episode numbers and confidence
    let re = regex::Regex::new(r"S(\d+)E(\d+)").unwrap();
    let caps = re.captures(answer)
        .ok_or_else(|| anyhow::anyhow!("Could not parse episode format from: {}", answer))?;
    
    let season: u32 = caps[1].parse()?;
    let episode: u32 = caps[2].parse()?;
    
    // Find matching episode for title
    let matched_ep = episodes.iter()
        .find(|ep| ep.season == season && ep.episode == episode)
        .ok_or_else(|| anyhow::anyhow!("Episode S{:02}E{:02} not found in metadata", season, episode))?;
    
    // Extract confidence - look for any number after the episode code
    let confidence_re = regex::Regex::new(r"(\d+)\s*$").unwrap();
    let confidence = if let Some(conf_caps) = confidence_re.captures(answer) {
        conf_caps[1].parse::<f32>().unwrap_or(85.0)
    } else {
        85.0  // Default to 85% if not specified
    };
    
    Ok(EpisodeMatch {
        season,
        episode,
        title: matched_ep.title.clone(),
        confidence,
    })
}
