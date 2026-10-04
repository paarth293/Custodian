//! Finds files with identical content.
//!
//! Files are grouped by size first (free), then by a hash of the first 64 KiB,
//! and only then by a full BLAKE3 hash, so most files are never fully read.

use crate::hashing::{hash_file, hash_prefix};
use crate::walk::{walk_files, FileEntry, WalkLimits, WalkStats};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

const PREFIX_BYTES: u64 = 64 * 1024;

/// A set of files with identical content. `files[0]` is the copy to keep:
/// the one closest to the folder root, then the oldest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub size: u64,
    pub hash: String,
    pub files: Vec<FileEntry>,
}

impl DuplicateGroup {
    /// Bytes recovered by removing every copy except the kept one.
    pub fn wasted_bytes(&self) -> u64 {
        self.size * (self.files.len() as u64 - 1)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DuplicateReport {
    /// Largest waste first.
    pub groups: Vec<DuplicateGroup>,
    pub stats: WalkStats,
    pub wasted_bytes: u64,
    pub extra_copies: usize,
}

/// Progress phases reported while searching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Scanning { files: usize },
    Comparing { done: usize, total: usize },
}

/// Searches `roots` for duplicate files.
pub fn find_duplicates(
    roots: &[PathBuf],
    limits: WalkLimits,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> DuplicateReport {
    let mut by_size: HashMap<u64, Vec<FileEntry>> = HashMap::new();
    let stats = walk_files(
        roots,
        limits,
        cancel,
        &mut |files| progress(Progress::Scanning { files }),
        &mut |entry| by_size.entry(entry.size).or_default().push(entry),
    );

    let candidates: Vec<Vec<FileEntry>> = by_size.into_values().filter(|g| g.len() > 1).collect();
    let total = candidates.iter().map(Vec::len).sum();
    let mut groups = Vec::new();

    for (index, same_size) in candidates.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let size = same_size[0].size;
        for same_prefix in group_by(same_size, |e| hash_prefix(&e.path, PREFIX_BYTES).ok()) {
            // Small files are fully covered by the prefix hash.
            let exact = if size <= PREFIX_BYTES {
                let hash = hash_file(&same_prefix[0].path).unwrap_or_default();
                vec![(hash, same_prefix)]
            } else {
                group_by_keyed(same_prefix, |e| hash_file(&e.path).ok())
            };
            for (hash, mut files) in exact {
                if files.len() > 1 && !hash.is_empty() {
                    files.sort_by(|a, b| {
                        let depth = |e: &FileEntry| e.path.components().count();
                        depth(a)
                            .cmp(&depth(b))
                            .then(a.modified.cmp(&b.modified))
                            .then(a.path.cmp(&b.path))
                    });
                    groups.push(DuplicateGroup { size, hash, files });
                }
            }
        }
        progress(Progress::Comparing {
            done: index + 1,
            total,
        });
    }

    groups.sort_by(|a, b| {
        b.wasted_bytes()
            .cmp(&a.wasted_bytes())
            .then(a.hash.cmp(&b.hash))
    });
    let wasted_bytes = groups.iter().map(DuplicateGroup::wasted_bytes).sum();
    let extra_copies = groups.iter().map(|g| g.files.len() - 1).sum();
    DuplicateReport {
        groups,
        stats,
        wasted_bytes,
        extra_copies,
    }
}

fn group_by_keyed(
    entries: Vec<FileEntry>,
    key: impl Fn(&FileEntry) -> Option<String>,
) -> Vec<(String, Vec<FileEntry>)> {
    let mut map: HashMap<String, Vec<FileEntry>> = HashMap::new();
    for entry in entries {
        if let Some(k) = key(&entry) {
            map.entry(k).or_default().push(entry);
        }
    }
    map.into_iter().filter(|(_, g)| g.len() > 1).collect()
}

fn group_by(
    entries: Vec<FileEntry>,
    key: impl Fn(&FileEntry) -> Option<String>,
) -> Vec<Vec<FileEntry>> {
    group_by_keyed(entries, key)
        .into_iter()
        .map(|(_, g)| g)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn report(root: &std::path::Path) -> DuplicateReport {
        find_duplicates(
            &[root.to_path_buf()],
            WalkLimits::default(),
            &AtomicBool::new(false),
            &mut |_| {},
        )
    }

    #[test]
    fn finds_identical_files_and_keeps_the_shallowest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("backup/old")).unwrap();
        fs::write(root.join("photo.jpg"), b"same picture bytes").unwrap();
        fs::write(root.join("backup/photo copy.jpg"), b"same picture bytes").unwrap();
        fs::write(root.join("backup/old/photo (1).jpg"), b"same picture bytes").unwrap();
        fs::write(root.join("other.jpg"), b"different bytes!!!").unwrap();

        let r = report(root);
        assert_eq!(r.groups.len(), 1);
        let group = &r.groups[0];
        assert_eq!(group.files.len(), 3);
        assert_eq!(group.files[0].path, root.join("photo.jpg"));
        assert_eq!(r.extra_copies, 2);
        assert_eq!(r.wasted_bytes, 2 * b"same picture bytes".len() as u64);
    }

    #[test]
    fn same_size_different_content_is_not_a_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.bin"), b"aaaa").unwrap();
        fs::write(dir.path().join("b.bin"), b"bbbb").unwrap();
        assert!(report(dir.path()).groups.is_empty());
    }

    #[test]
    fn large_files_differing_after_the_prefix_are_not_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = vec![7u8; 100 * 1024];
        let mut b = a.clone();
        a[90 * 1024] = 1;
        b[90 * 1024] = 2;
        fs::write(dir.path().join("a.bin"), &a).unwrap();
        fs::write(dir.path().join("b.bin"), &b).unwrap();
        fs::write(dir.path().join("a-copy.bin"), &a).unwrap();

        let r = report(dir.path());
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.groups[0].files.len(), 2);
    }

    #[test]
    fn orders_groups_by_wasted_space() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["s1", "s2"] {
            fs::write(dir.path().join(name), b"small").unwrap();
        }
        for name in ["b1", "b2"] {
            fs::write(dir.path().join(name), vec![9u8; 5000]).unwrap();
        }
        let r = report(dir.path());
        assert_eq!(r.groups.len(), 2);
        assert_eq!(r.groups[0].size, 5000);
    }
}
