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

/// Extract 1 minute of audio from the middle of the video and transcribe it
pub async fn extract_and_transcribe_audio(video_path: &Path) -> Result<String> {
    info!("Extracting audio from middle of {}", video_path.display());
    
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
    let middle_time = duration / 2.0;
    
    info!("Video duration: {:.0}s, extracting from {:.0}s", duration, middle_time);
    
    // Extract 1 minute of audio from the middle as WAV
    let audio_file = "/tmp/ripley_audio_sample.wav";
    let extract_result = Command::new("ffmpeg")
        .args(&[
            "-ss", &middle_time.to_string(),
            "-i", video_path.to_str().unwrap(),
            "-t", "60",  // 1 minute
            "-vn",  // No video
            "-acodec", "pcm_s16le",  // PCM WAV format
            "-ar", "16000",  // 16kHz sample rate (good for speech)
            "-ac", "1",  // Mono
            "-y",  // Overwrite
            audio_file
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;
    
    if !extract_result.success() {
        return Err(anyhow::anyhow!("Failed to extract audio"));
    }
    
    info!("Audio extracted, transcribing with Whisper...");
    
    // Use OpenAI Whisper API or local whisper to transcribe
    // For now, use whisper.cpp or openai-whisper Python package
    let transcript_output = Command::new("python3")
        .arg("-c")
        .arg(r#"
import whisper
import sys

model = whisper.load_model("base")
result = model.transcribe(sys.argv[1])
print(result["text"])
"#)
        .arg(audio_file)
        .output()
        .await;
    
    match transcript_output {
        Ok(output) if output.status.success() => {
            let transcript = String::from_utf8_lossy(&output.stdout).trim().to_string();
            info!("Transcription complete: {} characters", transcript.len());
            Ok(transcript)
        }
        _ => {
            warn!("Whisper not available, trying OpenAI API...");
            transcribe_with_openai_api(audio_file).await
        }
    }
}

/// Transcribe audio using OpenAI Whisper API
async fn transcribe_with_openai_api(audio_path: &str) -> Result<String> {
    // Check for OpenAI API key
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY not set"))?;
    
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
    
    // Check for OpenAI API key
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY not set for episode matching"))?;
    
    // Build episode list for context
    let episode_list: Vec<String> = episodes.iter()
        .map(|ep| format!("S{:02}E{:02}: {}", ep.season, ep.episode, ep.title))
        .collect();
    
    let prompt = format!(
        r#"You are matching a TV episode transcript to the correct episode.

Show: {}
Available episodes:
{}

Transcript sample from middle of episode:
"{}"

Based on this dialogue, which episode is this most likely from? Respond with ONLY the episode code (e.g., "S01E05") and confidence 0-100."#,
        show_name,
        episode_list.join("\n"),
        transcript.chars().take(500).collect::<String>()  // First 500 chars
    );
    
    debug!("Sending to OpenAI: {} chars", prompt.len());
    
    // Call OpenAI API
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [
                {"role": "system", "content": "You are a TV episode identification assistant. Respond with only the episode code and confidence."},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.3
        }))
        .send()
        .await?;
    
    let result: serde_json::Value = response.json().await?;
    let answer = result["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No response from OpenAI"))?;
    
    debug!("OpenAI response: {}", answer);
    
    // Parse response like "S02E05" or "S02E05 confidence: 85"
    let parts: Vec<&str> = answer.split_whitespace().collect();
    let episode_code = parts[0];
    
    // Parse S02E05 format
    if let Some(s_pos) = episode_code.find('S') {
        if let Some(e_pos) = episode_code.find('E') {
            let season: u32 = episode_code[s_pos+1..e_pos].parse()?;
            let episode: u32 = episode_code[e_pos+1..].parse()?;
            
            // Find matching episode for title
            let matched_ep = episodes.iter()
                .find(|ep| ep.season == season && ep.episode == episode)
                .ok_or_else(|| anyhow::anyhow!("Episode not found in metadata"))?;
            
            let confidence = if parts.len() > 2 {
                parts[2].parse::<f32>().unwrap_or(75.0)
            } else {
                75.0
            };
            
            Ok(EpisodeMatch {
                season,
                episode,
                title: matched_ep.title.clone(),
                confidence,
            })
        } else {
            Err(anyhow::anyhow!("Invalid episode format"))
        }
    } else {
        Err(anyhow::anyhow!("Invalid episode format"))
    }
}
