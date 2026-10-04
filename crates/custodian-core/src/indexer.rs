use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use custodian_winsec::{SafePath, PathError};

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
}

/// Scans a directory and returns file metadata.
/// Rejects any path that escapes the user's allowed roots (e.g. C:\Windows).
pub fn scan_directory(target: impl AsRef<Path>, root: impl AsRef<Path>) -> Result<Vec<FileInfo>, String> {
    // 1. Enforce safety using winsec's SafePath
    let safe_dir = SafePath::new(target, root)
        .map_err(|e| format!("Security violation: {}", e))?;
    
    let mut results = Vec::new();
    
    // 2. Read only if it's a directory
    if !safe_dir.as_path().is_dir() {
        return Ok(results);
    }
    
    // 3. Traverse and collect metadata
    for entry in fs::read_dir(safe_dir.as_path()).map_err(|e| e.to_string())? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // Skip inaccessible files quietly
        };
        
        let path = entry.path();
        
        // Skip subdirectories for this flat-scan implementation
        if path.is_dir() {
            continue;
        }
        
        if let Ok(metadata) = entry.metadata() {
            results.push(FileInfo {
                path: path.clone(),
                name: entry.file_name().to_string_lossy().to_string(),
                size_bytes: metadata.len(),
                modified: metadata.modified().ok(),
            });
        }
    }
    
    Ok(results)
}
