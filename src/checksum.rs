use anyhow::Result;
use sha2::{Sha256, Digest};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;
use walkdir::WalkDir;
use tracing::info;

/// Calculate SHA-256 checksum for all files in a directory
pub fn calculate_directory_checksum(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
    }

    let mut hasher = Sha256::new();
    let mut file_paths: Vec<PathBuf> = Vec::new();

    // Collect all file paths in sorted order for consistent checksums
    if path.is_file() {
        file_paths.push(path.to_path_buf());
    } else {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            file_paths.push(entry.path().to_path_buf());
        }
    }

    file_paths.sort();

    // Calculate combined checksum of all files
    for file_path in file_paths {
        let mut file = File::open(&file_path)?;
        let mut buffer = vec![0u8; 8192];

        // Include file path in hash for better integrity checking
        hasher.update(file_path.to_string_lossy().as_bytes());

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
    }

    let checksum = format!("{:x}", hasher.finalize());
    info!("Calculated checksum for {}: {}", path.display(), checksum);
    Ok(checksum)
}

/// Verify a directory checksum matches the expected value
pub fn verify_checksum(path: &Path, expected_checksum: &str) -> Result<bool> {
    let calculated = calculate_directory_checksum(path)?;
    Ok(calculated == expected_checksum)
}

