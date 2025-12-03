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
        .args([
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
        .args([
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
      let segments = [
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
            .args([
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

/// Extract audio from a specific segment (for retry attempts)
pub async fn extract_and_transcribe_audio_segment(
    video_path: &Path,
    start_time: f64,
    duration: f64,
) -> Result<String> {
    let audio_file = format!("/tmp/ripley_audio_retry_{}.wav", start_time as u64);
    
    // Extract segment
    let extract_result = Command::new("ffmpeg")
        .args([
            "-ss", &start_time.to_string(),
            "-i", video_path.to_str().unwrap(),
            "-t", &duration.to_string(),
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
        return Err(anyhow::anyhow!("Failed to extract audio segment"));
    }
    
    // Transcribe segment
    let transcript = transcribe_with_openai_api(&audio_file).await?;
    
    // Clean up temp file
    let _ = tokio::fs::remove_file(&audio_file).await;
    
    Ok(transcript)
}

/// Transcribe audio using OpenAI Whisper API (DISABLED)
async fn transcribe_with_openai_api(_audio_path: &str) -> Result<String> {
    Err(anyhow::anyhow!("OpenAI API support has been removed from Ripley"))
}

/// Match transcript against TMDB episodes using OpenAI
pub async fn match_episode_by_transcript(
    show_name: &str,
    transcript: &str,
    episodes: &[crate::dvd_metadata::Episode],
) -> Result<EpisodeMatch> {
    match_episode_by_transcript_with_exclusion(show_name, transcript, episodes, None).await
}

/// Match transcript against TMDB episodes using OpenAI, excluding a specific episode
pub async fn match_episode_by_transcript_with_exclusion(
    _show_name: &str,
    _transcript: &str,
    _episodes: &[crate::dvd_metadata::Episode],
    _exclude_episode: Option<(u32, u32)>, // (season, episode) to exclude
) -> Result<EpisodeMatch> {
    Err(anyhow::anyhow!("OpenAI API support has been removed from Ripley"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dvd_metadata::Episode;

    #[test]
    fn test_parse_srt_dialogue() {
        let srt_content = r#"1
00:00:01,000 --> 00:00:04,000
Hello there!

2
00:00:05,000 --> 00:00:08,000
This is a test subtitle.
"#;
        
        let dialogue = parse_srt_dialogue(srt_content);
        assert!(dialogue.contains("Hello there!"));
        assert!(dialogue.contains("This is a test subtitle."));
        assert!(!dialogue.contains("00:00:01"));
    }

    #[test]
    fn test_parse_srt_with_formatting() {
        let srt_content = r#"1
00:00:01,000 --> 00:00:04,000
<i>Italicized text</i>

2
00:00:05,000 --> 00:00:08,000
<b>Bold text</b> and normal
"#;
        
        let dialogue = parse_srt_dialogue(srt_content);
        assert!(dialogue.contains("Italicized text"));
        assert!(dialogue.contains("Bold text"));
        assert!(!dialogue.contains("<i>"));
        assert!(!dialogue.contains("</b>"));
    }

    #[test]
    fn test_episode_match_parsing() {
        let _episodes = vec![
            Episode {
                season: 1,
                episode: 1,
                title: "Pilot".to_string(),
                title_index: 0,
                runtime_minutes: Some(22),
                overview: Some("First episode".to_string()),
            },
            Episode {
                season: 1,
                episode: 2,
                title: "Second Episode".to_string(),
                title_index: 1,
                runtime_minutes: Some(22),
                overview: Some("Second episode".to_string()),
            },
        ];

        // Test parsing various response formats
        let test_cases = vec![
            ("S01E01\n90", 1, 1, 90.0),
            ("S01E02 confidence: 88", 1, 2, 88.0),
            ("S01E01\n95", 1, 1, 95.0),
        ];

        let re = regex::Regex::new(r"S(\d+)E(\d+)").unwrap();
        let confidence_re = regex::Regex::new(r"(\d+)").unwrap();
        for (response, expected_season, expected_episode, expected_confidence) in test_cases {
            let caps = re.captures(response).unwrap();
            assert_eq!(caps[1].parse::<u32>().unwrap(), expected_season);
            assert_eq!(caps[2].parse::<u32>().unwrap(), expected_episode);
            
            // Extract confidence - find number after episode code
            let after_episode = &response[caps.get(0).unwrap().end()..];
            let confidence = if let Some(conf_caps) = confidence_re.captures(after_episode) {
                conf_caps[1].parse::<f32>().unwrap_or(85.0)
            } else {
                85.0
            };
            assert_eq!(confidence, expected_confidence);
        }
    }

    #[test]
    fn test_episode_match_structure() {
        let ep_match = EpisodeMatch {
            season: 2,
            episode: 5,
            title: "Test Episode".to_string(),
            confidence: 92.5,
        };

        assert_eq!(ep_match.season, 2);
        assert_eq!(ep_match.episode, 5);
        assert_eq!(ep_match.title, "Test Episode");
        assert_eq!(ep_match.confidence, 92.5);
    }

    #[test]
    fn test_srt_empty_dialogue() {
        let empty_srt = "";
        let dialogue = parse_srt_dialogue(empty_srt);
        assert_eq!(dialogue.trim(), "");
    }

    #[test]
    fn test_srt_multiline_dialogue() {
        let srt_content = r#"1
00:00:01,000 --> 00:00:04,000
This is the first line
And this is the second line
Even a third line!
"#;
        
        let dialogue = parse_srt_dialogue(srt_content);
        assert!(dialogue.contains("This is the first line"));
        assert!(dialogue.contains("And this is the second line"));
        assert!(dialogue.contains("Even a third line!"));
    }
}
