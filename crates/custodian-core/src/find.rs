//! "Where's my file?" (§22): finds files by the words in their names.

use crate::walk::{walk_files, FileEntry, WalkLimits, WalkStats};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::SystemTime;

/// Matches kept before sorting, so a vague query cannot use unbounded memory.
const MAX_MATCHES: usize = 5000;

/// The lowercase words of a query: "Tax-Return 2025.pdf" gives
/// ["tax", "return", "2025", "pdf"].
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Files under `roots` whose names contain every word of `query`, in any
/// case and order, newest first, at most `count` of them.
pub fn find_by_name(
    roots: &[PathBuf],
    query: &str,
    count: usize,
    limits: WalkLimits,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(usize),
) -> (Vec<FileEntry>, WalkStats) {
    let wanted = words(query);
    let mut matches = Vec::new();
    let stats = walk_files(roots, limits, cancel, progress, &mut |entry| {
        if wanted.is_empty() || matches.len() >= MAX_MATCHES {
            return;
        }
        let name = entry
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if wanted.iter().all(|w| name.contains(w.as_str())) {
            matches.push(entry);
        }
    });
    matches.sort_by(|a, b| {
        b.modified
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .cmp(&a.modified.unwrap_or(SystemTime::UNIX_EPOCH))
            .then(a.path.cmp(&b.path))
    });
    matches.truncate(count);
    (matches, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    fn find(root: &std::path::Path, query: &str) -> Vec<String> {
        let (files, _) = find_by_name(
            &[root.to_path_buf()],
            query,
            10,
            WalkLimits::default(),
            &AtomicBool::new(false),
            &mut |_| {},
        );
        files
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn matches_every_word_in_any_case_and_order() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("work/old")).unwrap();
        for name in [
            "Resume_2026.pdf",
            "work/old/resume-draft.docx",
            "taxes 2025.xlsx",
            "notes.txt",
        ] {
            fs::write(dir.path().join(name), b"x").unwrap();
        }
        let mut found = find(dir.path(), "RESUME");
        found.sort();
        assert_eq!(found, vec!["Resume_2026.pdf", "resume-draft.docx"]);
        assert_eq!(find(dir.path(), "2025 taxes"), vec!["taxes 2025.xlsx"]);
        assert_eq!(find(dir.path(), "resume pdf"), vec!["Resume_2026.pdf"]);
        assert!(find(dir.path(), "invoice").is_empty());
        assert!(
            find(dir.path(), "  ...  ").is_empty(),
            "no words, no matches"
        );
    }

    #[test]
    fn lists_the_newest_first_and_stops_at_count() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..15 {
            let path = dir.path().join(format!("photo_{i:02}.jpg"));
            fs::write(&path, b"x").unwrap();
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + i * 60);
            fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(when)
                .unwrap();
        }
        let found = find(dir.path(), "photo");
        assert_eq!(found.len(), 10);
        assert_eq!(found[0], "photo_14.jpg");
        assert_eq!(found[9], "photo_05.jpg");
    }
}
