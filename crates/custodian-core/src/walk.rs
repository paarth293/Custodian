//! Bounded, cancellable directory walking for user folders.
//!
//! Never follows symlinks or junctions, skips system and dependency folders,
//! and stops after a file budget so a scan can never run away.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

/// Folder names that are never scanned: system areas, caches and dependency
/// trees that are full of intentional duplicates.
pub const SKIPPED_DIRS: &[&str] = &[
    "$recycle.bin",
    "system volume information",
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "appdata",
    "node_modules",
    ".git",
    ".svn",
    "target",
    "__pycache__",
    ".venv",
    "venv",
];

/// Metadata for one regular file found during a walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy)]
pub struct WalkLimits {
    /// Stop after visiting this many files.
    pub max_files: usize,
    /// Ignore files smaller than this many bytes.
    pub min_size: u64,
}

impl Default for WalkLimits {
    fn default() -> Self {
        Self {
            max_files: 400_000,
            min_size: 1,
        }
    }
}

/// Summary of a finished walk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WalkStats {
    pub files_seen: usize,
    /// True if the walk stopped early (file budget or cancellation).
    pub truncated: bool,
}

/// Hidden folders (starting with a dot) and the folders in [`SKIPPED_DIRS`].
fn is_skipped(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with('.') || SKIPPED_DIRS.contains(&lower.as_str())
}

/// Visits every regular file under `roots`. `progress` is called every
/// 1,000 files with the running count.
pub fn walk_files(
    roots: &[PathBuf],
    limits: WalkLimits,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(usize),
    visit: &mut dyn FnMut(FileEntry),
) -> WalkStats {
    let mut stats = WalkStats::default();
    let mut stack: Vec<PathBuf> = roots.iter().rev().cloned().collect();

    while let Some(dir) = stack.pop() {
        if cancel.load(Ordering::Relaxed) || stats.files_seen >= limits.max_files {
            stats.truncated = true;
            break;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue; // Unreadable folder: skip quietly.
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                if !is_skipped(&entry.file_name().to_string_lossy()) {
                    stack.push(path);
                }
                continue;
            }
            if !file_type.is_file() || is_reparse_point(&path) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            stats.files_seen += 1;
            if stats.files_seen.is_multiple_of(1000) {
                progress(stats.files_seen);
            }
            if meta.len() >= limits.min_size {
                visit(FileEntry {
                    path,
                    size: meta.len(),
                    modified: meta.modified().ok(),
                });
            }
            if stats.files_seen >= limits.max_files {
                stats.truncated = true;
                break;
            }
        }
    }
    stats
}

/// OneDrive placeholders and other reparse points are not real local files.
#[cfg(windows)]
fn is_reparse_point(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
    fs::symlink_metadata(path)
        .map(|m| {
            m.file_attributes()
                & (FILE_ATTRIBUTE_REPARSE_POINT
                    | FILE_ATTRIBUTE_OFFLINE
                    | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
                != 0
        })
        .unwrap_or(true)
}

#[cfg(not(windows))]
fn is_reparse_point(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(roots: &[PathBuf], limits: WalkLimits) -> (Vec<PathBuf>, WalkStats) {
        let mut found = Vec::new();
        let stats = walk_files(
            roots,
            limits,
            &AtomicBool::new(false),
            &mut |_| {},
            &mut |e| found.push(e.path),
        );
        found.sort();
        (found, stats)
    }

    #[test]
    fn walks_nested_folders_and_skips_ignored_ones() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::create_dir_all(root.join("node_modules/x")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join("top.txt"), b"1").unwrap();
        fs::write(root.join("a/b/deep.txt"), b"2").unwrap();
        fs::write(root.join("node_modules/x/dep.js"), b"3").unwrap();
        fs::write(root.join(".git/HEAD"), b"4").unwrap();

        let (found, stats) = collect(&[root.to_path_buf()], WalkLimits::default());
        assert_eq!(found, vec![root.join("a/b/deep.txt"), root.join("top.txt")]);
        assert!(!stats.truncated);
    }

    #[test]
    fn honours_minimum_size_and_file_budget() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            fs::write(dir.path().join(format!("{i}.txt")), vec![0u8; i * 10]).unwrap();
        }
        let roots = [dir.path().to_path_buf()];
        let (found, _) = collect(
            &roots,
            WalkLimits {
                max_files: 100,
                min_size: 20,
            },
        );
        assert_eq!(found.len(), 3);

        let (found, stats) = collect(
            &roots,
            WalkLimits {
                max_files: 2,
                min_size: 0,
            },
        );
        assert_eq!(found.len(), 2);
        assert!(stats.truncated);
    }

    #[test]
    fn stops_when_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"x").unwrap();
        let mut count = 0;
        let stats = walk_files(
            &[dir.path().to_path_buf()],
            WalkLimits::default(),
            &AtomicBool::new(true),
            &mut |_| {},
            &mut |_| count += 1,
        );
        assert_eq!(count, 0);
        assert!(stats.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn never_follows_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), b"x").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        let (found, _) = collect(&[dir.path().to_path_buf()], WalkLimits::default());
        assert!(found.is_empty());
    }
}
