use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::config::Config;
use crate::speech_match;

#[derive(Debug, Default)]
pub struct CostTracker {
    pub whisper_minutes: f64,
    pub gpt_requests: usize,
}

impl CostTracker {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_whisper_minutes(&mut self, minutes: f64) {
        self.whisper_minutes += minutes;
    }
    
    pub fn add_gpt_request(&mut self) {
        self.gpt_requests += 1;
    }
    
    /// Calculate estimated total cost
    /// Whisper: $0.006 per minute
    /// GPT-4o-mini: ~$0.00015 per request (input + output tokens)
    pub fn estimate_total_cost(&self) -> f64 {
        let whisper_cost = self.whisper_minutes * 0.006;
        let gpt_cost = self.gpt_requests as f64 * 0.00015;
        whisper_cost + gpt_cost
    }
    
    pub fn estimate_whisper_cost(&self) -> f64 {
        self.whisper_minutes * 0.006
    }
    
    pub fn estimate_gpt_cost(&self) -> f64 {
        self.gpt_requests as f64 * 0.00015
    }
}

/// Run the rename command on a directory of video files
pub async fn run_rename(
    directory: Option<PathBuf>,
    title: Option<String>,
    skip_speech: bool,
    skip_filebot: bool,
) -> Result<()> {
    let config = Config::load()?;
    
    // Get working directory
    let work_dir = directory.unwrap_or_else(|| std::env::current_dir().unwrap());
    println!("📁 Working directory: {}", work_dir.display());
    
    // Get show title (prompt if not provided)
    let show_title = match title {
        Some(t) => t,
        None => {
            print!("Enter TV show title: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };
    
    if show_title.is_empty() {
        anyhow::bail!("Show title is required");
    }
    
    println!("📺 Show: {}", show_title);
    
    // Fetch episode metadata from TMDB
    println!("🔍 Fetching episode list from TMDB...");
    let metadata = crate::dvd_metadata::fetch_dvd_metadata("", Some(&show_title))
        .await
        .context("Failed to fetch TMDB metadata")?;
    
    let unique_seasons: std::collections::HashSet<u32> = metadata.episodes.iter()
        .map(|ep| ep.season)
        .collect();
    println!("✓ Found {} episodes across {} seasons", 
        metadata.episodes.len(), 
        unique_seasons.len()
    );
    
    // Find all .mkv files in directory
    let mkv_files: Vec<PathBuf> = WalkDir::new(&work_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("mkv"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();
    
    if mkv_files.is_empty() {
        println!("⚠️  No .mkv files found in directory");
        return Ok(());
    }
    
    println!("📹 Found {} video file(s) to process", mkv_files.len());
    
    let mut cost_tracker = CostTracker::new();
    let mut renamed_count = 0;
    
    // Phase 1: Speech matching
    if !skip_speech && config.speech_match.enabled {
        println!("\n🎙️  Phase 1: Speech Matching");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        for (i, file_path) in mkv_files.iter().enumerate() {
            let file_name = file_path.file_name().unwrap().to_string_lossy();
            println!("\n[{}/{}] {}", i + 1, mkv_files.len(), file_name);
            
            // Skip if already in correct format (ShowName.S##E##.Title.mkv)
            if is_already_renamed(&file_name) {
                println!("  ↪ Already renamed, skipping");
                continue;
            }
            
            // Extract and transcribe audio
            print!("  ⏳ Extracting audio...");
            io::stdout().flush()?;
            
            let result = speech_match::extract_and_transcribe_audio(file_path).await;
            
            match result {
                Ok(transcript) => {
                    println!(" ✓ Transcribed {} characters", transcript.len());
                    
                    // Track Whisper cost only if we used audio transcription (not subtitles)
                    // Subtitle extraction uses 0 Whisper minutes
                    if transcript.len() < 2000 {
                        // Likely audio transcription (3 segments x 1.5 min = 4.5 min)
                        cost_tracker.add_whisper_minutes(4.5);
                    }
                    // Otherwise it was subtitle extraction (free)
                    
                    // Match episode
                    print!("  🔍 Matching episode...");
                    io::stdout().flush()?;
                    
                    let match_result = speech_match::match_episode_by_transcript(
                        &show_title,
                        &transcript,
                        &metadata.episodes,
                    )
                    .await;
                    
                    cost_tracker.add_gpt_request();
                    
                    match match_result {
                        Ok(episode_match) => {
                            println!(
                                " ✓ S{:02}E{:02}: {} (confidence: {}%)",
                                episode_match.season,
                                episode_match.episode,
                                episode_match.title,
                                episode_match.confidence
                            );
                            
                            // Rename file
                            if episode_match.confidence >= 85.0 {
                                let new_name = format!(
                                    "{}.S{:02}E{:02}.{}.mkv",
                                    sanitize_filename(&show_title).replace(' ', "."),
                                    episode_match.season,
                                    episode_match.episode,
                                    sanitize_filename(&episode_match.title).replace(' ', ".")
                                );
                                
                                let new_path = file_path.with_file_name(&new_name);
                                
                                match std::fs::rename(file_path, &new_path) {
                                    Ok(_) => {
                                        println!("  ✓ Renamed to: {}", new_name);
                                        renamed_count += 1;
                                    }
                                    Err(e) => {
                                        println!("  ✗ Rename failed: {}", e);
                                    }
                                }
                            } else {
                                println!("  ⚠️  Confidence too low, skipping rename");
                            }
                        }
                        Err(e) => {
                            println!(" ✗ Match failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!(" ✗ Transcription failed: {}", e);
                }
            }
        }
    } else if skip_speech {
        println!("\n⏭️  Skipping speech matching phase");
    }
    
    // Phase 2: Filebot (if enabled and not skipped)
    if !skip_filebot && !config.filebot.skip_by_default {
        println!("\n🤖 Phase 2: Filebot Duration Matching");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Run filebot on the entire directory - let it verify/fix all files
        let filebot_result = crate::filebot::rename_with_filebot(
            &work_dir,
            &show_title,
            |msg| {
                println!("{}", msg);
            },
        )
        .await;
        
        match filebot_result {
            Ok(_) => {
                println!("✓ Filebot completed successfully");
            }
            Err(e) => {
                println!("⚠️  Filebot failed: {}", e);
            }
        }
    } else if skip_filebot {
        println!("\n⏭️  Skipping Filebot phase");
    }
    
    // Output summary
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 Summary");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✓ Renamed {} file(s)", renamed_count);
    println!("\n💰 Cost Estimate:");
    println!("   Whisper: {:.1} min @ $0.006/min = ${:.4}", 
        cost_tracker.whisper_minutes,
        cost_tracker.estimate_whisper_cost()
    );
    println!("   GPT-4o-mini: {} requests @ ~$0.00015/req = ${:.4}", 
        cost_tracker.gpt_requests,
        cost_tracker.estimate_gpt_cost()
    );
    println!("   ────────────────────────────────");
    println!("   Total: ${:.4}", cost_tracker.estimate_total_cost());
    
    Ok(())
}

/// Check if filename is already in the correct format (contains S##E##)
fn is_already_renamed(filename: &str) -> bool {
    // Look for pattern like S01E02 or S1E2
    let re = regex::Regex::new(r"[Ss]\d{1,2}[Ee]\d{1,2}").unwrap();
    re.is_match(filename)
}

/// Sanitize filename by removing invalid characters
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_tracker() {
        let mut tracker = CostTracker::new();
        tracker.add_whisper_minutes(10.0);
        tracker.add_gpt_request();
        
        // 10 minutes * $0.006 = $0.06
        assert!((tracker.estimate_whisper_cost() - 0.06).abs() < 0.001);
        
        // 1 request * $0.00015 = $0.00015
        assert!((tracker.estimate_gpt_cost() - 0.00015).abs() < 0.0001);
        
        // Total should be $0.06015
        assert!((tracker.estimate_total_cost() - 0.06015).abs() < 0.001);
    }
    
    #[test]
    fn test_is_already_renamed() {
        assert!(is_already_renamed("Show.S01E02.Title.mkv"));
        assert!(is_already_renamed("Show.s01e02.Title.mkv"));
        assert!(is_already_renamed("Show.S1E2.Title.mkv"));
        assert!(!is_already_renamed("Show.01.Title.mkv"));
        assert!(!is_already_renamed("Show.Title.mkv"));
    }
    
    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            sanitize_filename("Foster's Home: The Movie?"),
            "Foster's Home_ The Movie_"
        );
        assert_eq!(
            sanitize_filename("Normal Title"),
            "Normal Title"
        );
    }
}
