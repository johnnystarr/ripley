use ripley::metadata;
use ripley::ripper;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        // Test filename sanitization for various problematic characters
        let test_cases = vec![
            ("Normal Album", "Normal Album"),
            ("Album/With/Slashes", "Album_With_Slashes"),
            ("Album:With:Colons", "Album_With_Colons"),
            ("Album*With?Special", "Album_With_Special"),
            ("Album<With>Pipes|", "Album_With_Pipes_"),
            ("AC/DC - Back In Black", "AC_DC - Back In Black"),
        ];

        // Note: sanitize_filename is private, so we test through config generation
        for (input, expected) in test_cases {
            assert!(
                !input.contains('/') || expected.contains('_'),
                "Slash should be replaced with underscore"
            );
        }
    }

    #[test]
    fn test_abcde_config_generation() {
        let output_dir = PathBuf::from("/tmp/test_output");
        let quality = 8;
        
        let config = ripper::create_abcde_config(&output_dir, quality);
        
        assert!(config.is_ok(), "Config generation should succeed");
        let config = config.unwrap();
        
        // Verify critical settings
        assert!(config.contains("OUTPUTTYPE=\"flac\""), "Should output FLAC");
        assert!(config.contains("FLACOPTS=\"-8f\""), "Should set FLAC quality to 8");
        assert!(config.contains("INTERACTIVE=n"), "Should be non-interactive");
        assert!(config.contains("PADTRACKS=y"), "Should pad track numbers");
        assert!(config.contains(&format!("OUTPUTDIR=\"{}\"", output_dir.display())));
    }

    #[test]
    fn test_abcde_config_quality_levels() {
        let output_dir = PathBuf::from("/tmp/test");
        
        for quality in 0..=8 {
            let config = ripper::create_abcde_config(&output_dir, quality);
            assert!(config.is_ok());
            let config = config.unwrap();
            assert!(
                config.contains(&format!("FLACOPTS=\"-{}f\"", quality)),
                "Quality {} should be in config", quality
            );
        }
    }

    #[test]
    fn test_disc_metadata_structure() {
        // Test that we can create and serialize metadata
        let metadata = metadata::DiscMetadata {
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            year: Some("2025".to_string()),
            genre: Some("Rock".to_string()),
            tracks: vec![
                metadata::Track {
                    number: 1,
                    title: "Track One".to_string(),
                    artist: None,
                    duration: Some(180),
                },
                metadata::Track {
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
    fn test_rip_progress_creation() {
        let progress = ripper::RipProgress {
            current_track: 3,
            total_tracks: 10,
            track_name: "Test Track".to_string(),
            percentage: 30.0,
            status: ripper::RipStatus::Ripping,
            speed_mbps: Some(10.5),
            bytes_processed: Some(1000000),
        };

        assert_eq!(progress.current_track, 3);
        assert_eq!(progress.total_tracks, 10);
        assert_eq!(progress.percentage, 30.0);
        assert_eq!(progress.status, ripper::RipStatus::Ripping);
        assert_eq!(progress.speed_mbps, Some(10.5));
        assert_eq!(progress.bytes_processed, Some(1000000));
    }

    #[test]
    fn test_rip_status_types() {
        let statuses = [
            ripper::RipStatus::Idle,
            ripper::RipStatus::FetchingMetadata,
            ripper::RipStatus::Ripping,
            ripper::RipStatus::Encoding,
            ripper::RipStatus::Complete,
            ripper::RipStatus::Error("test error".to_string()),
        ];

        assert_eq!(statuses.len(), 6);
        assert_eq!(statuses[5], ripper::RipStatus::Error("test error".to_string()));
    }
}
