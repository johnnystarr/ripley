use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use tracing::info;

/// Calculate SHA-256 checksum of a file
pub fn calculate_file_checksum(file_path: &Path) -> Result<String> {
    let file = File::open(file_path)
        .with_context(|| format!("Failed to open file: {}", file_path.display()))?;
    
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    
    let mut buffer = vec![0u8; 8192]; // 8KB buffer
    
    loop {
        let bytes_read = reader.read(&mut buffer)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;
        
        if bytes_read == 0 {
            break;
        }
        
        hasher.update(&buffer[..bytes_read]);
    }
    
    let hash = hasher.finalize();
    let hex_hash = hex::encode(hash);
    
    Ok(hex_hash)
}

/// Calculate SHA-256 checksum of a directory (recursively hashes all files)
pub fn calculate_directory_checksum(dir_path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut file_paths = Vec::new();
    
    // Collect all files in the directory
    if dir_path.is_dir() {
        for entry in walkdir::WalkDir::new(dir_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            file_paths.push(entry.path().to_path_buf());
        }
    }
    
    // Sort paths for consistent hashing
    file_paths.sort();
    
    info!("Calculating checksum for {} files in {}", file_paths.len(), dir_path.display());
    
    // Hash each file and include its relative path
    for file_path in file_paths {
        let relative_path = file_path.strip_prefix(dir_path)
            .unwrap_or(&file_path)
            .to_string_lossy();
        
        // Include path in hash for better uniqueness
        hasher.update(relative_path.as_bytes());
        
        // Hash file contents
        if let Ok(file_hash) = calculate_file_checksum(&file_path) {
            hasher.update(file_hash.as_bytes());
        }
    }
    
    let hash = hasher.finalize();
    let hex_hash = hex::encode(hash);
    
    Ok(hex_hash)
}

/// Verify a file's checksum matches expected value
#[allow(dead_code)]
pub fn verify_checksum(path: &Path, expected_checksum: &str) -> Result<bool> {
    let calculated = calculate_file_checksum(path)?;
    Ok(calculated == expected_checksum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_calculate_file_checksum() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, b"Hello, World!").unwrap();
        
        let checksum = calculate_file_checksum(&test_file).unwrap();
        
        // SHA-256 of "Hello, World!" 
        let expected = "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f";
        assert_eq!(checksum, expected);
    }

    #[test]
    fn test_calculate_file_checksum_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty.txt");
        fs::write(&test_file, b"").unwrap();
        
        let checksum = calculate_file_checksum(&test_file).unwrap();
        
        // SHA-256 of empty string
        let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(checksum, expected);
    }

    #[test]
    fn test_calculate_directory_checksum() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create multiple files
        fs::write(temp_dir.path().join("file1.txt"), b"Content 1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), b"Content 2").unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        fs::write(temp_dir.path().join("subdir").join("file3.txt"), b"Content 3").unwrap();
        
        let checksum = calculate_directory_checksum(temp_dir.path()).unwrap();
        
        // Checksum should be consistent for same directory structure
        let checksum2 = calculate_directory_checksum(temp_dir.path()).unwrap();
        assert_eq!(checksum, checksum2);
    }

    #[test]
    fn test_calculate_directory_checksum_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        
        let checksum = calculate_directory_checksum(temp_dir.path()).unwrap();
        assert!(!checksum.is_empty());
    }

    #[test]
    fn test_verify_checksum() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, b"Test content").unwrap();
        
        let expected_checksum = calculate_file_checksum(&test_file).unwrap();
        assert!(verify_checksum(&test_file, &expected_checksum).unwrap());
        assert!(!verify_checksum(&test_file, "wrong_checksum").unwrap());
    }
}
