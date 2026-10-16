//! Cancellable local file discovery with a separate conservative cleanup policy.
//!
//! Never follows symlinks or junctions. Cleanup uses a file budget and excludes
//! system/dependency folders; explicit read-only discovery can inspect all local
//! folders without truncating a whole-system request at the cleanup budget.

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
    /// Read-only discovery may inspect system and hidden folders. Mutation
    /// eligibility is separate and must never be inferred from this flag.
    pub include_system: bool,
}

impl Default for WalkLimits {
    fn default() -> Self {
        Self {
            max_files: 400_000,
            min_size: 0,
            include_system: false,
        }
    }
}

impl WalkLimits {
    pub fn whole_system_read() -> Self {
        Self {
            max_files: usize::MAX,
            min_size: 0,
            include_system: true,
        }
    }
}

/// Summary of a finished walk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WalkStats {
    pub files_seen: usize,
    /// Folders skipped due to permission or I/O errors.
    pub access_denied_count: usize,
    /// True if the walk stopped because file budget limit was reached.
    pub budget_exhausted: bool,
    /// True if the walk stopped due to cancellation.
    pub cancelled: bool,
    /// True if the walk stopped early (file budget or cancellation).
    pub truncated: bool,
    pub excluded_count: usize,
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
        if cancel.load(Ordering::Relaxed) {
            stats.cancelled = true;
            stats.truncated = true;
            break;
        }
        if stats.files_seen >= limits.max_files {
            stats.budget_exhausted = true;
            stats.truncated = true;
            break;
        }
        if crate::privacy::check(&dir).is_err() || is_reparse_point(&dir) {
            stats.excluded_count += 1;
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => {
                stats.access_denied_count += 1;
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    stats.access_denied_count += 1;
                    continue;
                }
            };
            if cancel.load(Ordering::Relaxed) {
                stats.cancelled = true;
                stats.truncated = true;
                break;
            }
            let path = entry.path();
            if crate::privacy::check(&path).is_err() {
                stats.excluded_count += 1;
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                stats.excluded_count += 1;
                continue;
            }
            if is_reparse_point(&path) {
                stats.excluded_count += 1;
                continue;
            }
            if file_type.is_dir() {
                if limits.include_system || !is_skipped(&entry.file_name().to_string_lossy()) {
                    stack.push(path);
                } else {
                    stats.excluded_count += 1;
                }
                continue;
            }
            if !file_type.is_file() {
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
                stats.budget_exhausted = true;
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
fn is_reparse_point(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(true)
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
                ..WalkLimits::default()
            },
        );
        assert_eq!(found.len(), 3);

        let (found, stats) = collect(
            &roots,
            WalkLimits {
                max_files: 2,
                min_size: 0,
                ..WalkLimits::default()
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
        assert!(stats.cancelled);
    }

    #[test]
    fn discovers_zero_byte_files_by_default() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("empty.txt"), b"").unwrap();
        let (found, _) = collect(&[dir.path().to_path_buf()], WalkLimits::default());
        assert_eq!(found, vec![dir.path().join("empty.txt")]);
    }

    #[test]
    fn whole_system_read_includes_system_hidden_and_custom_content() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "Windows",
            "Program Files",
            "AppData",
            ".private",
            "WorkProjects",
        ] {
            fs::create_dir(dir.path().join(name)).unwrap();
            fs::write(dir.path().join(name).join("payload"), b"read only").unwrap();
        }
        let (files, stats) = collect(&[dir.path().into()], WalkLimits::whole_system_read());
        assert_eq!(files.len(), 5);
        assert_eq!(stats.excluded_count, 0);
        for file in files {
            assert_eq!(fs::read(file).unwrap(), b"read only");
        }
    }

    #[test]
    fn records_cancellation_and_budget_exhaustion() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            fs::write(dir.path().join(format!("file_{i}.txt")), b"hello").unwrap();
        }
        let roots = [dir.path().to_path_buf()];
        let (_, stats) = collect(
            &roots,
            WalkLimits {
                max_files: 2,
                min_size: 0,
                ..WalkLimits::default()
            },
        );
        assert!(stats.budget_exhausted);
        assert!(stats.truncated);
        assert!(!stats.cancelled);

        let cancel = AtomicBool::new(true);
        let stats_cancelled = walk_files(
            &roots,
            WalkLimits::default(),
            &cancel,
            &mut |_| {},
            &mut |_| {},
        );
        assert!(stats_cancelled.cancelled);
        assert!(stats_cancelled.truncated);
        assert!(!stats_cancelled.budget_exhausted);
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
