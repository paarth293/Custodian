//! Finds the largest files under a set of folders.

use crate::walk::{walk_files, FileEntry, WalkLimits, WalkStats};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

/// Returns up to `count` files, biggest first.
pub fn largest_files(
    roots: &[PathBuf],
    count: usize,
    limits: WalkLimits,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(usize),
) -> (Vec<FileEntry>, WalkStats) {
    // Min-heap of (size, path) keeps only the `count` biggest seen so far.
    let mut heap: BinaryHeap<Reverse<(u64, PathBuf)>> = BinaryHeap::new();
    let mut entries = std::collections::HashMap::new();
    let stats = walk_files(roots, limits, cancel, progress, &mut |entry| {
        if count == 0 {
            return;
        }
        if heap.len() < count {
            heap.push(Reverse((entry.size, entry.path.clone())));
            entries.insert(entry.path.clone(), entry);
        } else if let Some(Reverse((smallest, _))) = heap.peek() {
            if entry.size > *smallest {
                if let Some(Reverse((_, evicted))) = heap.pop() {
                    entries.remove(&evicted);
                }
                heap.push(Reverse((entry.size, entry.path.clone())));
                entries.insert(entry.path.clone(), entry);
            }
        }
    });
    let mut files: Vec<FileEntry> = entries.into_values().collect();
    files.sort_by(|a, b| b.size.cmp(&a.size).then(a.path.cmp(&b.path)));
    (files, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_biggest_first() {
        let dir = tempfile::tempdir().unwrap();
        for (name, size) in [("a", 10), ("b", 500), ("c", 50), ("d", 1000), ("e", 5)] {
            std::fs::write(dir.path().join(name), vec![0u8; size]).unwrap();
        }
        let (files, _) = largest_files(
            &[dir.path().to_path_buf()],
            3,
            WalkLimits::default(),
            &AtomicBool::new(false),
            &mut |_| {},
        );
        let sizes: Vec<u64> = files.iter().map(|f| f.size).collect();
        assert_eq!(sizes, vec![1000, 500, 50]);
    }
}
