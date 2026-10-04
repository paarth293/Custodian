//! Wording of Cubby's replies. Pure functions, so every message is testable.

use custodian_core::cleanup::{CleanupOutcome, HistoryEntry, UndoOutcome};
use custodian_core::duplicates::DuplicateReport;
use custodian_core::walk::FileEntry;
use std::path::Path;

pub const GREETING: &str =
    "Hi! I'm Cubby. I can find duplicate files, show what's using your space, \
clean up safely (with undo), and open apps, folders and websites. \
Try \"find duplicates in Downloads\" or \"open myntra\".";

pub const WHO_AM_I: &str =
    "I'm Cubby, your Custodian companion. I live on your desktop, keep your files tidy \
without ever deleting the only copy of anything, and everything I change can be undone.";

pub const HELP: &str = "Here's what I can do right now:\n\
• Find duplicate files: \"find duplicates\" or \"duplicates in Pictures\"\n\
• Show what's using space: \"biggest files in Downloads\", \"how much free space?\"\n\
• Clean up safely: approve my plan and the extra copies go to my safe place\n\
• Undo: \"undo\" puts the last clean-up back\n\
• History: \"what did you do?\"\n\
• Open things: \"open notepad\", \"open downloads\", \"open myntra\", \"search cheap flights\"\n\
• Time and date: \"what time is it?\"\n\
For everything else I use my local AI brain, when one is installed.";

pub fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        one
    } else {
        many
    }
}

/// 12345 → "12,345".
pub fn count(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Human-readable size: "1.4 GB", "820 KB".
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// "A", "A and B", "A, B and C".
pub fn join_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

pub fn title_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn parent_name(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn duplicates_summary(report: &DuplicateReport, folders: &[&str]) -> String {
    let scanned = count(report.stats.files_seen);
    let place = join_list(folders);
    if report.groups.is_empty() {
        return format!(
            "Good news: I checked {scanned} files in {place} and found no duplicate files."
        );
    }
    let mut text = format!(
        "I checked {scanned} files in {place} and found {} extra {} of {} {}, using {}.",
        count(report.extra_copies),
        plural(report.extra_copies, "copy", "copies"),
        count(report.groups.len()),
        plural(report.groups.len(), "file", "files"),
        bytes(report.wasted_bytes),
    );
    if report.stats.truncated {
        text.push_str(" (I stopped early because there were a lot of files.)");
    }
    text.push_str("\n\nBiggest ones:");
    for group in report.groups.iter().take(5) {
        text.push_str(&format!(
            "\n• {} — {} copies, {} each",
            file_name(&group.files[0].path),
            group.files.len(),
            bytes(group.size)
        ));
    }
    text.push_str(
        "\n\nI'll keep the original of each and move the extra copies to my safe place. Review the plan below.",
    );
    text
}

pub fn largest_summary(files: &[FileEntry], folders: &[&str], scanned: usize) -> String {
    if files.is_empty() {
        return format!("I didn't find any files in {}.", join_list(folders));
    }
    let mut text = format!(
        "The biggest files in {} (out of {} checked):",
        join_list(folders),
        count(scanned)
    );
    for f in files {
        text.push_str(&format!(
            "\n• {} — {} (in {})",
            file_name(&f.path),
            bytes(f.size),
            parent_name(&f.path)
        ));
    }
    text
}

/// `drives`: (name, free bytes, total bytes).
pub fn disk_summary(drives: &[(String, u64, u64)]) -> String {
    if drives.is_empty() {
        return "I couldn't read your drives' free space.".into();
    }
    let mut text = String::from("Here's your storage:");
    for (name, free, total) in drives {
        let used_pct = if *total > 0 {
            (total - free) as f64 / *total as f64 * 100.0
        } else {
            0.0
        };
        text.push_str(&format!(
            "\n• {name} — {} free of {} ({used_pct:.0}% used)",
            bytes(*free),
            bytes(*total)
        ));
    }
    text
}

pub fn cleanup_summary(outcome: &CleanupOutcome) -> String {
    let mut text = if outcome.moved == 0 {
        "I didn't move anything.".to_string()
    } else {
        format!(
            "Done! I moved {} duplicate {} to my safe place and freed {}. Say \"undo\" anytime to put them back.",
            count(outcome.moved),
            plural(outcome.moved, "copy", "copies"),
            bytes(outcome.bytes)
        )
    };
    if !outcome.skipped.is_empty() {
        text.push_str(&format!(
            " I left {} {} alone because {} changed since the scan.",
            count(outcome.skipped.len()),
            plural(outcome.skipped.len(), "file", "files"),
            plural(outcome.skipped.len(), "it", "they")
        ));
    }
    text
}

pub fn undo_summary(outcome: &UndoOutcome) -> String {
    let mut text = format!(
        "Done! I put back {} {} exactly where {} were.",
        count(outcome.restored),
        plural(outcome.restored, "file", "files"),
        plural(outcome.restored, "it", "they")
    );
    if !outcome.skipped.is_empty() {
        text.push_str(&format!(
            " {} {} stayed in my safe place because a new file is in {} spot now.",
            count(outcome.skipped.len()),
            plural(outcome.skipped.len(), "file", "files"),
            plural(outcome.skipped.len(), "its", "their")
        ));
    }
    text
}

pub fn undo_error(err: &str) -> String {
    if err.contains("nothing to undo") {
        "There's nothing to undo — I haven't changed anything yet.".into()
    } else {
        format!("I couldn't undo that: {err}")
    }
}

pub fn history_line(entry: &HistoryEntry) -> String {
    let when = chrono::DateTime::from_timestamp(entry.created_unix as i64, 0)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%-d %b, %-I:%M %p")
                .to_string()
        })
        .unwrap_or_default();
    format!(
        "{when}: moved {} {} ({}) to the safe place{}",
        count(entry.files),
        plural(entry.files, "duplicate", "duplicates"),
        bytes(entry.bytes),
        if entry.undone { " — undone" } else { "" }
    )
}

pub fn history_summary(history: &[HistoryEntry]) -> String {
    if history.is_empty() {
        return "I haven't changed anything yet.".into();
    }
    let mut text = String::from("Here's what I've done recently:");
    for entry in history.iter().take(5) {
        text.push_str(&format!("\n• {}", history_line(entry)));
    }
    text
}

pub fn no_brain(reason: &str) -> String {
    format!(
        "I'm not sure how to help with that yet. My local AI brain isn't available ({reason}). \
Here's what I can already do — say \"help\" to see it all: find duplicates, show big files and free space, \
clean up with undo, and open apps, folders and websites."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use custodian_core::duplicates::DuplicateGroup;
    use custodian_core::walk::WalkStats;
    use std::path::PathBuf;

    fn entry(path: &str, size: u64) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            size,
            modified: None,
        }
    }

    #[test]
    fn formats_numbers_and_sizes() {
        assert_eq!(count(7), "7");
        assert_eq!(count(1234567), "1,234,567");
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
        assert_eq!(join_list(&["A", "B", "C"]), "A, B and C");
        assert_eq!(title_case("myntra"), "Myntra");
    }

    #[test]
    fn summarises_duplicates() {
        let report = DuplicateReport {
            groups: vec![DuplicateGroup {
                size: 2048,
                hash: "h".into(),
                files: vec![
                    entry("/u/photo.jpg", 2048),
                    entry("/u/b/photo (1).jpg", 2048),
                ],
            }],
            stats: WalkStats {
                files_seen: 1500,
                truncated: false,
            },
            wasted_bytes: 2048,
            extra_copies: 1,
        };
        let text = duplicates_summary(&report, &["Downloads"]);
        assert!(text.starts_with(
            "I checked 1,500 files in Downloads and found 1 extra copy of 1 file, using 2.0 KB."
        ));
        assert!(text.contains("photo.jpg — 2 copies, 2.0 KB each"));
    }

    #[test]
    fn says_so_when_there_are_no_duplicates() {
        let text = duplicates_summary(&DuplicateReport::default(), &["Pictures"]);
        assert!(text.contains("found no duplicate files"));
    }

    #[test]
    fn describes_disk_space() {
        let gb = 1024 * 1024 * 1024;
        let text = disk_summary(&[("C:\\".into(), 100 * gb, 400 * gb)]);
        assert!(text.contains("C:\\ — 100 GB free of 400 GB (75% used)"));
    }

    #[test]
    fn explains_skipped_files_after_cleanup() {
        let outcome = CleanupOutcome {
            plan_id: "p".into(),
            moved: 2,
            bytes: 2048,
            skipped: vec![(PathBuf::from("x"), "changed".into())],
        };
        let text = cleanup_summary(&outcome);
        assert!(text.contains("moved 2 duplicate copies"));
        assert!(text.contains("left 1 file alone because it changed"));
    }
}
