#![allow(dead_code)]
//! Wording of Cubby's replies. Pure functions, so every message is testable.

use super::apps::OpenApp;
use super::system::Snapshot;
use custodian_core::cleanup::{CleanupOutcome, HistoryEntry, LeftOut, PurgeOutcome, UndoOutcome};
use custodian_core::duplicates::DuplicateReport;
use custodian_core::walk::{FileEntry, WalkStats};
use std::path::Path;
use std::time::{Duration, SystemTime};

pub const GREETING: &str =
    "Hi! I'm Cubby. I can find duplicate files, show what's using your space, \
clean up safely (with undo), and open apps, folders and websites. \
Try \"find duplicates in Downloads\" or \"open myntra\".";

pub const WHO_AM_I: &str =
    "I'm Cubby, your Custodian companion. I live on your desktop, keep your files tidy \
without ever deleting the only copy of anything, and everything I change can be undone.";

pub const HELP: &str = "Here's what I can do right now:
• Suggestions: \"any suggestions?\" for a quick check-up of your PC
• Privacy: \"is my data safe?\" shows how I protect it
• Find a file: \"where's my resume?\"
• Check on your PC: \"is my PC slow?\", \"how much free space?\"
• Show what's using space: \"biggest files in Downloads\"
• Find duplicate files: \"find duplicates\" or \"duplicates in Pictures\"
• Clean up safely: approve my plan and the extra copies wait in my safe place for 30 days
• Clean temporary files: \"clean temp files\"
• Undo: \"undo\" puts the last clean-up back
• Free that space now: \"empty the safe place\"
• History: \"what did you do?\"
• Open things: \"open spotify\", \"open my resume\", \"open downloads\", \"open myntra\", \"search cheap flights\"
• Close apps: \"close notepad\", \"what's open?\"
• Reminders and to-dos: \"remind me to call mom at 5pm\", \"add milk to my list\", \"what's on my list?\", \"mark 1 as done\"
• Time and date: \"what time is it?\"
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

/// Invisible formatting characters that can make text display differently
/// from what it is: bidirectional overrides and isolates (the "RTLO" trick
/// that shows "invoice" + U+202E + "fdp.exe" as `invoiceexe.pdf`) and zero-width
/// characters (SR-I2, RT-09).
pub fn is_hidden_format(c: char) -> bool {
    matches!(
        c,
        '\u{061C}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

/// A name from the disk made safe to show: control and invisible formatting
/// characters are replaced with a visible marker instead of being obeyed.
pub fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_control() || is_hidden_format(c) {
                '\u{FFFD}'
            } else {
                c
            }
        })
        .collect()
}

/// The display-safe file name of `path`.
pub fn file_name(path: &Path) -> String {
    safe_name(
        &path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string()),
    )
}

/// The display-safe name of the folder holding `path`.
pub fn parent_name(path: &Path) -> String {
    safe_name(
        &path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
}

/// The whole path, display-safe.
pub fn safe_path(path: &Path) -> String {
    safe_name(&path.display().to_string())
}

/// "today", "yesterday", "3 days ago" or "on 4 Mar 2025", by local calendar date.
pub fn ago(when: SystemTime, now: SystemTime) -> String {
    let day = |t: SystemTime| chrono::DateTime::<chrono::Local>::from(t).date_naive();
    match (day(now) - day(when)).num_days() {
        0 => "today".into(),
        1 => "yesterday".into(),
        n @ 2..=6 => format!("{n} days ago"),
        _ => format!(
            "on {}",
            chrono::DateTime::<chrono::Local>::from(when).format("%-d %b %Y")
        ),
    }
}

/// "45 minutes", "5 hours", "1 day", "12 days".
pub fn duration_words(d: Duration) -> String {
    let minutes = d.as_secs() / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    if days >= 1 {
        format!("{days} {}", plural(days as usize, "day", "days"))
    } else if hours >= 1 {
        format!("{hours} {}", plural(hours as usize, "hour", "hours"))
    } else {
        format!(
            "{minutes} {}",
            plural(minutes as usize, "minute", "minutes")
        )
    }
}

/// The answer to "where's my …?".
pub fn find_summary(
    query: &str,
    files: &[FileEntry],
    folders: &[&str],
    stats: WalkStats,
    now: SystemTime,
) -> String {
    let query = safe_name(query);
    if files.is_empty() {
        let mut text = format!(
            "I couldn't find a file with \"{query}\" in its name in {} ({} files checked).",
            join_list(folders),
            count(stats.files_seen)
        );
        if stats.truncated {
            text.push_str(" I stopped early, so it may still be somewhere I didn't look.");
        }
        return with_coverage(text, &stats);
    }
    let mut text = format!(
        "I found {} with \"{query}\" in the name{}:",
        if files.len() == 1 {
            "1 file".to_string()
        } else {
            format!("{} files", count(files.len()))
        },
        if files.len() > 1 {
            ", newest first"
        } else {
            ""
        }
    );
    for f in files {
        let changed = f
            .modified
            .map(|m| format!(", changed {}", ago(m, now)))
            .unwrap_or_default();
        text.push_str(&format!(
            "\n• {} — in {}{changed}\n  Path: {}\n  Size: {} bytes",
            file_name(&f.path),
            parent_name(&f.path),
            safe_path(&f.path),
            f.size,
        ));
    }
    text.push_str(&format!(
        "\nSearched {} ({} files checked).",
        join_list(folders),
        count(stats.files_seen)
    ));
    with_coverage(text, &stats)
}

fn with_coverage(mut text: String, stats: &WalkStats) -> String {
    if stats.excluded_count > 0 {
        text.push_str(&format!("\nSkipped {} links, cloud placeholders or excluded folders. Results cover accessible local files only.", count(stats.excluded_count)));
    }
    if stats.access_denied_count > 0 {
        text.push_str(&format!(
            "\nCould not inspect {} entries or locations; this coverage is incomplete.",
            count(stats.access_denied_count)
        ));
    }
    text
}

/// The answer to "is my PC slow?", with plain advice when something stands out.
pub fn system_summary(s: &Snapshot) -> String {
    let mut text = String::from("Here's how your PC is doing right now:");
    text.push_str(&format!("\n• Processor: {:.0}% busy", s.cpu_percent));
    if s.memory_total > 0 {
        text.push_str(&format!(
            "\n• Memory: {} of {} in use ({:.0}%)",
            bytes(s.memory_used),
            bytes(s.memory_total),
            s.memory_percent()
        ));
    }
    let top: Vec<String> = s
        .apps
        .iter()
        .take(3)
        .map(|a| format!("{} ({})", a.name, bytes(a.memory_bytes)))
        .collect();
    if !top.is_empty() {
        let top: Vec<&str> = top.iter().map(String::as_str).collect();
        text.push_str(&format!("\n• Using the most memory: {}", join_list(&top)));
    }
    if let Some(b) = &s.battery {
        let mut line = format!("\n• Battery: {}%", b.percent);
        if b.plugged_in {
            line.push_str(", plugged in");
        } else {
            line.push_str(", on battery");
            if let Some(minutes) = b.minutes_left {
                line.push_str(&format!(
                    ", about {} left",
                    duration_words(Duration::from_secs(u64::from(minutes) * 60))
                ));
            }
        }
        if b.saver_on {
            line.push_str(" (battery saver is on)");
        }
        text.push_str(&line);
    }
    text.push_str(&format!("\n• Switched on for {}", duration_words(s.uptime)));

    let mut tips = Vec::new();
    if s.memory_percent() >= 85.0 {
        match s.apps.first() {
            Some(top) => tips.push(format!(
                "Memory is nearly full, which slows everything down. Closing {} would free the most.",
                top.name
            )),
            None => tips.push("Memory is nearly full, which slows everything down.".into()),
        }
    }
    if s.cpu_percent >= 80.0 {
        match s.busiest_app().filter(|a| a.cpu_percent >= 10.0) {
            Some(busy) => tips.push(format!(
                "The processor is very busy, mostly with {}.",
                busy.name
            )),
            None => tips.push("The processor is very busy right now.".into()),
        }
    }
    if s.uptime >= Duration::from_secs(7 * 24 * 3600) {
        tips.push(format!(
            "Your PC has been on for {}. A restart often clears up slowness.",
            duration_words(s.uptime)
        ));
    }
    if let Some(b) = &s.battery {
        if b.saver_on {
            tips.push("Battery saver slows the PC down a little to save power.".into());
        }
        if !b.plugged_in && b.percent <= 20 {
            tips.push("The battery is low, so plug in soon.".into());
        }
    }
    if tips.is_empty() {
        text.push_str("\n\nNothing looks wrong right now.");
    } else {
        text.push_str("\n\n");
        text.push_str(&tips.join(" "));
    }
    text
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
    text
}

/// Why some duplicates are not in the plan: they are in a synced folder.
pub fn left_out_note(left: &LeftOut) -> String {
    let places: Vec<&str> = left.places.iter().map(String::as_str).collect();
    format!(
        " I left out {} {} ({}) in {}: removing {} there would also remove {} from the cloud and your other devices.",
        count(left.files),
        plural(left.files, "copy", "copies"),
        bytes(left.bytes),
        join_list(&places),
        plural(left.files, "it", "them"),
        plural(left.files, "it", "them"),
    )
}

pub fn largest_summary(files: &[FileEntry], folders: &[&str], stats: &WalkStats) -> String {
    if files.is_empty() {
        let mut text = format!("I didn't find any files in {}.", join_list(folders));
        if stats.access_denied_count > 0 {
            text.push_str(&format!(
                " (Skipped {} inaccessible {} due to permission restrictions.)",
                count(stats.access_denied_count),
                plural(stats.access_denied_count, "location", "locations")
            ));
        }
        if stats.budget_exhausted || stats.truncated {
            text.push_str(" (I stopped early because of the search limit.)");
        }
        return with_coverage(text, stats);
    }
    let heading = if files.len() == 1 {
        format!(
            "The largest file in {} (out of {} checked):",
            join_list(folders),
            count(stats.files_seen)
        )
    } else {
        format!(
            "The biggest files in {} (out of {} checked):",
            join_list(folders),
            count(stats.files_seen)
        )
    };
    let mut text = heading;
    for f in files {
        text.push_str(&format!(
            "\n• {} — {} ({} bytes) in {}",
            file_name(&f.path),
            bytes(f.size),
            f.size,
            safe_path(&f.path)
        ));
    }
    if stats.budget_exhausted || stats.truncated {
        text.push_str(
            "\n\nNote: I stopped early due to the scan limit, so this is a partial result.",
        );
    }
    if stats.access_denied_count > 0 {
        text.push_str(&format!(
            "\nNote: Skipped {} inaccessible {} due to permission restrictions.",
            count(stats.access_denied_count),
            plural(stats.access_denied_count, "location", "locations")
        ));
    }
    if stats.cancelled {
        text.push_str("\nNote: The search was cancelled before completing.");
    }
    with_coverage(text, stats)
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

/// A local calendar date such as "4 November".
pub fn day_month(unix: u64) -> String {
    chrono::DateTime::from_timestamp(unix as i64, 0)
        .map(|t| t.with_timezone(&chrono::Local).format("%-d %B").to_string())
        .unwrap_or_default()
}

/// After a clean-up. Moving files into the safe place frees no space yet
/// (EV-01), so the reply says when it will, and how to get them back.
pub fn cleanup_summary(outcome: &CleanupOutcome) -> String {
    let mut text = if outcome.moved == 0 {
        "I didn't move anything.".to_string()
    } else {
        format!(
            "Done! I moved {} duplicate {} ({}) to my safe place; the originals are untouched. \
The space frees up on {}. Until then, say \"undo\" to put {} back, or \"empty the safe place\" to free the space now.",
            count(outcome.moved),
            plural(outcome.moved, "copy", "copies"),
            bytes(outcome.bytes),
            day_month(outcome.frees_unix),
            plural(outcome.moved, "it", "them"),
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
    if outcome.restored == 0 && outcome.skipped.is_empty() {
        return "There was nothing left to put back.".into();
    }
    let mut parts = Vec::new();
    if outcome.restored > 0 {
        let n = outcome.restored;
        if outcome.beside.is_empty() {
            parts.push(format!(
                "Done! I put back {} {} exactly where {} {}.",
                count(n),
                plural(n, "file", "files"),
                plural(n, "it", "they"),
                plural(n, "was", "were")
            ));
        } else {
            let names: Vec<String> = outcome
                .beside
                .iter()
                .take(3)
                .map(|(_, to)| format!("\"{}\"", file_name(to)))
                .collect();
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            parts.push(format!(
                "Done! I put back {} {}. {} went next to {} old spot as {}, because a new file has that name now. Nothing was overwritten.",
                count(n),
                plural(n, "file", "files"),
                if outcome.beside.len() == n {
                    plural(n, "It", "They").to_string()
                } else {
                    format!("{} of them", count(outcome.beside.len()))
                },
                plural(outcome.beside.len(), "its", "their"),
                join_list(&names)
            ));
        }
    }
    if let Some((_, reason)) = outcome.skipped.first() {
        let m = outcome.skipped.len();
        parts.push(format!(
            "I couldn't put back {} {} ({reason}). {} still safe; say \"undo\" to try again.",
            count(m),
            plural(m, "file", "files"),
            plural(m, "It's", "They're")
        ));
    }
    parts.join(" ")
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
    let state = if entry.undone {
        " — undone".to_string()
    } else if entry.waiting_files > 0 && entry.purged_files > 0 {
        format!(
            " — {} freed, {} more on {}",
            bytes(entry.purged_bytes),
            bytes(entry.waiting_bytes),
            day_month(entry.frees_unix)
        )
    } else if entry.waiting_files > 0 {
        format!(" — space frees on {}", day_month(entry.frees_unix))
    } else if entry.purged_files > 0 {
        " — space freed".to_string()
    } else {
        String::new()
    };
    format!(
        "{when}: moved {} {} ({}) to the safe place{state}",
        count(entry.files),
        plural(entry.files, "duplicate", "duplicates"),
        bytes(entry.bytes),
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

/// The question before emptying the safe place.
pub fn empty_question(files: usize, total: u64) -> String {
    format!(
        "Free {} now by deleting the {} {} in Cubby's safe place?\n\n\
Each is an extra copy of a file you still have. I check every one against its original first and keep any that no longer match. \
Afterwards, \"undo\" can only re-create a copy from its original.",
        bytes(total),
        count(files),
        plural(files, "copy", "copies"),
    )
}

/// After the safe place was emptied, by the person or when the window ended.
pub fn purge_summary(outcome: &PurgeOutcome, by_person: bool) -> String {
    let mut parts = Vec::new();
    if outcome.purged > 0 {
        parts.push(format!(
            "{}I freed {} by deleting {} duplicate {} from my safe place. The originals are all still where they were.",
            if by_person { "Done! " } else { "" },
            bytes(outcome.bytes),
            count(outcome.purged),
            plural(outcome.purged, "copy", "copies"),
        ));
    } else if by_person && outcome.kept.is_empty() {
        parts.push("My safe place is already empty.".to_string());
    }
    if !outcome.kept.is_empty() {
        let k = outcome.kept.len();
        parts.push(format!(
            "I kept {} {} because {} original changed or is gone, so {} might be the only {} left. Say \"undo\" to put {} back.",
            count(k),
            plural(k, "copy", "copies"),
            plural(k, "its", "their"),
            plural(k, "it", "they"),
            plural(k, "one", "ones"),
            plural(k, "it", "them"),
        ));
    }
    parts.join(" ")
}

/// After scanning the temp folder.
pub fn temp_found(files: usize, total: u64) -> String {
    format!(
        "I found {} temporary {} ({}) that apps left behind more than a week ago. Review the plan below.",
        count(files),
        plural(files, "file", "files"),
        bytes(total)
    )
}

/// The plan card for deleting temporary files: it says out loud that this
/// cannot be undone (Rule 3).
pub fn temp_plan_summary(files: usize, total: u64) -> String {
    format!(
        "Delete {} temporary {} ({}). Apps recreate these if they need them. They are deleted for good, not moved to my safe place, so this can't be undone.",
        count(files),
        plural(files, "file", "files"),
        bytes(total)
    )
}

/// The approval question for deleting temporary files.
pub fn temp_question(files: usize, total: u64) -> String {
    format!(
        "Delete {} temporary {} ({}) for good?\n\nApps left {} behind more than a week ago and recreate {} if needed. This frees the space now and can't be undone.",
        count(files),
        plural(files, "file", "files"),
        bytes(total),
        plural(files, "it", "them"),
        plural(files, "it", "them"),
    )
}

/// After deleting temporary files.
pub fn temp_summary(outcome: &custodian_core::temp::TempOutcome) -> String {
    let mut text = if outcome.deleted == 0 {
        "I didn't delete anything.".to_string()
    } else {
        format!(
            "Done! I deleted {} temporary {} and freed {}.",
            count(outcome.deleted),
            plural(outcome.deleted, "file", "files"),
            bytes(outcome.bytes)
        )
    };
    if outcome.skipped > 0 {
        text.push_str(&format!(
            " I left {} alone because {} in use or changed since I looked.",
            count(outcome.skipped),
            plural(outcome.skipped, "it's", "they're")
        ));
    }
    text
}

/// When several files match "open my …".
pub fn which_file(name: &str, files: &[&FileEntry]) -> String {
    let mut text = format!(
        "I found {} files named like \"{}\":",
        count(files.len()),
        safe_name(name)
    );
    for (i, f) in files.iter().take(5).enumerate() {
        text.push_str(&format!(
            "\n{}. {} (in {})",
            i + 1,
            file_name(&f.path),
            parent_name(&f.path)
        ));
    }
    text.push_str("\nSay \"open\" and the full name to choose one.");
    text
}

/// What Cubby can say about its protections, all checked just now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protections {
    /// `Some(true)` sandboxed, `Some(false)` not (with the reason, if known),
    /// `None` when the AI is not running right now.
    pub ai_sandboxed: Option<bool>,
    pub sandbox_problem: Option<String>,
    /// Windows Hello guards approvals (otherwise a Windows dialog does).
    pub hello: bool,
    /// The history log matches its sealed anchor.
    pub history_intact: bool,
    /// Cubby is hidden from screen sharing and screenshots right now.
    pub hidden_from_capture: bool,
    /// Cloud-synced folders Cubby leaves alone, such as "OneDrive".
    pub synced: Vec<String>,
}

/// The answer to "is my data safe?": only what is true on this PC now.
pub fn security_summary(p: &Protections) -> String {
    let mut lines = vec![
        "Everything I know stays on this PC. I have no cloud account, and I never upload your files or what you tell me.".to_string(),
    ];
    lines.push(match (p.ai_sandboxed, &p.sandbox_problem) {
        (Some(true), _) => "My AI brain runs in a Windows sandbox: it can't use the internet or read your files.".into(),
        (Some(false), Some(problem)) => format!(
            "My AI brain couldn't start in its Windows sandbox on this PC ({}), so it runs without one. It still can't change anything by itself.",
            safe_name(problem)
        ),
        (Some(false), None) => "My AI brain runs without its Windows sandbox right now. It still can't change anything by itself.".into(),
        (None, _) => "My AI brain isn't running right now. When it starts, it runs in a Windows sandbox where possible, with no internet and no access to your files.".into(),
    });
    lines.push(if p.hello {
        "Nothing is moved or deleted without your OK, and clean-ups need Windows Hello (your face, fingerprint or PIN).".into()
    } else {
        "Nothing is moved or deleted without your OK in a Windows dialog. Setting up Windows Hello would make that even safer.".into()
    });
    lines.push(
        "Duplicates go to my safe place for 30 days first, and \"undo\" brings them back.".into(),
    );
    if !p.synced.is_empty() {
        let names: Vec<&str> = p.synced.iter().map(String::as_str).collect();
        lines.push(format!(
            "I never move or delete anything in {}, so nothing changes on your other devices.",
            join_list(&names)
        ));
    }
    lines.push(if p.history_intact {
        "My history log is tamper-evident and checks out.".into()
    } else {
        "Heads up: my history log was changed outside Custodian. I've noted it; your files are not affected.".into()
    });
    if p.hidden_from_capture {
        lines.push("I'm hidden from screen sharing and screenshots, so file names never show on a shared screen.".into());
    }
    format!(
        "Here's how I keep your things safe:\n• {}",
        lines.join("\n• ")
    )
}

/// The answer to "what apps are open?".
pub fn open_apps_summary(apps: &[OpenApp]) -> String {
    if apps.is_empty() {
        return "You don't have any apps open right now, apart from me.".into();
    }
    let names: Vec<String> = apps
        .iter()
        .map(|a| {
            let name = safe_name(&a.name);
            match a.windows.len() {
                1 => name,
                n => format!("{name} ({n} windows)"),
            }
        })
        .collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    format!(
        "You have {} {} open: {}.",
        count(apps.len()),
        plural(apps.len(), "app", "apps"),
        join_list(&names)
    )
}

/// After asking an app to close: `asked` windows were asked, `still_open`
/// are still there (usually because the app is asking to save work).
pub fn closed_summary(name: &str, asked: usize, still_open: usize) -> String {
    let name = safe_name(name);
    if still_open == 0 {
        return if asked == 1 {
            format!("Closed {name}.")
        } else {
            format!("Closed {} {name} windows.", count(asked))
        };
    }
    format!(
        "I asked {name} to close. {} still open, probably asking whether to save your work, so answer {} there.",
        if still_open == 1 {
            "It's".to_string()
        } else {
            format!("{} windows are", count(still_open))
        },
        plural(still_open, "it", "them")
    )
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
                excluded_count: 0,
                access_denied_count: 0,
                budget_exhausted: false,
                cancelled: false,
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
            frees_unix: 0,
            skipped: vec![(PathBuf::from("x"), "changed".into())],
        };
        let text = cleanup_summary(&outcome);
        assert!(text.contains("moved 2 duplicate copies"));
        assert!(text.contains("left 1 file alone because it changed"));
    }

    #[test]
    fn never_claims_space_was_freed_before_it_is() {
        let outcome = CleanupOutcome {
            plan_id: "p".into(),
            moved: 3,
            bytes: 3 * 1024 * 1024,
            frees_unix: 1_700_000_000,
            skipped: vec![],
        };
        let text = cleanup_summary(&outcome);
        assert!(!text.contains("freed"), "{text}");
        assert!(text.contains("The space frees up on "), "{text}");
        assert!(text.contains(&day_month(1_700_000_000)), "{text}");
        assert!(text.contains("\"undo\""), "{text}");
    }

    #[test]
    fn explains_where_files_went_back() {
        let plain = UndoOutcome {
            plan_id: "p".into(),
            restored: 2,
            beside: vec![],
            skipped: vec![],
        };
        assert_eq!(
            undo_summary(&plain),
            "Done! I put back 2 files exactly where they were."
        );
        let beside = UndoOutcome {
            restored: 2,
            beside: vec![(
                PathBuf::from("C:\\u\\a.jpg"),
                PathBuf::from("C:\\u\\a (restored).jpg"),
            )],
            ..plain.clone()
        };
        let text = undo_summary(&beside);
        assert!(
            text.contains("1 of them went next to its old spot as \"a (restored).jpg\""),
            "{text}"
        );
        assert!(text.contains("Nothing was overwritten"), "{text}");
        let stuck = UndoOutcome {
            restored: 0,
            beside: vec![],
            skipped: vec![(PathBuf::from("x"), "access denied".into())],
            ..plain
        };
        let text = undo_summary(&stuck);
        assert!(
            text.starts_with("I couldn't put back 1 file (access denied)."),
            "{text}"
        );
        assert!(text.contains("try again"), "{text}");
    }

    #[test]
    fn history_shows_when_space_frees() {
        let mut entry = HistoryEntry {
            plan_id: "p".into(),
            created_unix: 1_700_000_000,
            files: 4,
            bytes: 4096,
            waiting_files: 4,
            waiting_bytes: 4096,
            frees_unix: 1_702_592_000,
            ..Default::default()
        };
        assert!(history_line(&entry).ends_with(&format!(
            "moved 4 duplicates (4.0 KB) to the safe place — space frees on {}",
            day_month(1_702_592_000)
        )));
        entry.waiting_files = 0;
        entry.purged_files = 4;
        assert!(history_line(&entry).ends_with("— space freed"));
        entry.undone = true;
        assert!(history_line(&entry).ends_with("— undone"));
    }

    #[test]
    fn says_only_what_is_true_about_security() {
        let mut p = Protections {
            ai_sandboxed: Some(true),
            sandbox_problem: None,
            hello: true,
            history_intact: true,
            hidden_from_capture: true,
            synced: vec!["OneDrive".into()],
        };
        let text = security_summary(&p);
        assert!(
            text.contains("runs in a Windows sandbox: it can't use the internet"),
            "{text}"
        );
        assert!(text.contains("need Windows Hello"), "{text}");
        assert!(
            text.contains("I never move or delete anything in OneDrive"),
            "{text}"
        );
        assert!(text.contains("checks out"), "{text}");
        p.ai_sandboxed = Some(false);
        p.sandbox_problem = Some("EPERM".into());
        p.hello = false;
        p.history_intact = false;
        p.hidden_from_capture = false;
        let text = security_summary(&p);
        assert!(
            text.contains("couldn't start in its Windows sandbox on this PC (EPERM)"),
            "{text}"
        );
        assert!(text.contains("Setting up Windows Hello"), "{text}");
        assert!(text.contains("was changed outside Custodian"), "{text}");
        assert!(!text.contains("hidden from screen sharing"), "{text}");
    }

    #[test]
    fn describes_open_apps_and_closing() {
        use custodian_winsec::apps::AppWindow;
        let window = |h: isize| AppWindow {
            handle: h,
            pid: 1,
            process_generation: Some(1),
            title: "t".into(),
            exe: "x.exe".into(),
        };
        let apps = vec![
            OpenApp {
                name: "Chrome".into(),
                windows: vec![window(1), window(2)],
            },
            OpenApp {
                name: "Notepad".into(),
                windows: vec![window(3)],
            },
        ];
        assert_eq!(
            open_apps_summary(&apps),
            "You have 2 apps open: Chrome (2 windows) and Notepad."
        );
        assert!(open_apps_summary(&[]).contains("apart from me"));
        assert_eq!(closed_summary("Notepad", 1, 0), "Closed Notepad.");
        assert_eq!(closed_summary("Chrome", 3, 0), "Closed 3 Chrome windows.");
        let waiting = closed_summary("Word", 1, 1);
        assert!(
            waiting.starts_with("I asked Word to close. It's still open"),
            "{waiting}"
        );
        assert!(waiting.contains("save your work"), "{waiting}");
    }

    #[test]
    fn reports_purges_and_what_was_kept() {
        let outcome = PurgeOutcome {
            purged: 5,
            bytes: 5 * 1024 * 1024,
            kept: vec![(
                PathBuf::from("x"),
                "the copy that was kept has changed".into(),
            )],
        };
        let text = purge_summary(&outcome, true);
        assert!(
            text.starts_with("Done! I freed 5.0 MB by deleting 5 duplicate copies"),
            "{text}"
        );
        assert!(
            text.contains("I kept 1 copy because its original changed"),
            "{text}"
        );
        assert_eq!(
            purge_summary(&PurgeOutcome::default(), true),
            "My safe place is already empty."
        );
        assert_eq!(purge_summary(&PurgeOutcome::default(), false), "");
        assert!(empty_question(2, 2048).starts_with("Free 2.0 KB now by deleting the 2 copies"));
    }

    #[test]
    fn shows_deceptive_names_as_they_really_are() {
        // "invoice<RLO>fdp.exe" would display as "invoiceexe.pdf".
        assert_eq!(
            safe_name("invoice\u{202e}fdp.exe"),
            "invoice\u{fffd}fdp.exe"
        );
        assert_eq!(safe_name("a\u{200b}b\nc"), "a\u{fffd}b\u{fffd}c");
        assert_eq!(safe_name("Résumé 2026.pdf"), "Résumé 2026.pdf");
        assert_eq!(
            file_name(Path::new("C:\\Users\\me\\x\u{202e}gpj.exe")),
            "x\u{fffd}gpj.exe"
        );
    }

    #[test]
    fn describes_when_and_how_long() {
        let now = SystemTime::now();
        let day = Duration::from_secs(24 * 3600);
        assert_eq!(ago(now, now), "today");
        assert_eq!(ago(now - 3 * day, now), "3 days ago");
        assert!(ago(now - 400 * day, now).starts_with("on "));
        assert_eq!(duration_words(Duration::from_secs(45 * 60)), "45 minutes");
        assert_eq!(duration_words(Duration::from_secs(3600)), "1 hour");
        assert_eq!(duration_words(6 * day), "6 days");
    }

    #[test]
    fn lists_found_files_or_says_where_it_looked() {
        let now = SystemTime::now();
        let files = vec![FileEntry {
            path: PathBuf::from("C:\\Users\\me\\Documents\\Resume_2026.pdf"),
            size: 10,
            modified: Some(now),
        }];
        let stats = WalkStats {
            files_seen: 1200,
            excluded_count: 0,
            access_denied_count: 0,
            budget_exhausted: false,
            cancelled: false,
            truncated: false,
        };
        let text = find_summary("resume", &files, &["Documents"], stats, now);
        assert!(
            text.starts_with("I found 1 file with \"resume\" in the name:"),
            "{text}"
        );
        assert!(
            text.contains("Resume_2026.pdf — in Documents, changed today"),
            "{text}"
        );

        let none = find_summary("tax", &[], &["Documents", "Desktop"], stats, now);
        assert_eq!(
            none,
            "I couldn't find a file with \"tax\" in its name in Documents and Desktop (1,200 files checked)."
        );
    }

    #[test]
    fn formats_largest_file_summary_with_exact_bytes_and_caveats() {
        let single = vec![FileEntry {
            path: PathBuf::from("D:\\huge_archive.zip"),
            size: 5_242_880_000,
            modified: None,
        }];
        let stats = WalkStats {
            files_seen: 45000,
            excluded_count: 0,
            access_denied_count: 3,
            budget_exhausted: true,
            cancelled: false,
            truncated: false,
        };
        let text = largest_summary(&single, &["D:\\"], &stats);
        assert!(
            text.starts_with("The largest file in D:\\ (out of 45,000 checked):"),
            "{text}"
        );
        assert!(
            text.contains("huge_archive.zip — 4.9 GB (5242880000 bytes) in D:\\huge_archive.zip"),
            "{text}"
        );
        assert!(
            text.contains(
                "Note: I stopped early due to the scan limit, so this is a partial result."
            ),
            "{text}"
        );
        assert!(
            text.contains("Note: Skipped 3 inaccessible locations due to permission restrictions."),
            "{text}"
        );
    }

    #[test]
    fn explains_the_system_state_with_advice() {
        use super::super::system::AppUsage;
        use custodian_winsec::power::Battery;
        let gb = 1024 * 1024 * 1024;
        let busy = Snapshot {
            cpu_percent: 91.0,
            memory_used: 13 * gb,
            memory_total: 14 * gb,
            apps: vec![
                AppUsage {
                    name: "Chrome".into(),
                    memory_bytes: 3 * gb,
                    cpu_percent: 50.0,
                },
                AppUsage {
                    name: "Teams".into(),
                    memory_bytes: gb,
                    cpu_percent: 5.0,
                },
            ],
            battery: Some(Battery {
                percent: 15,
                plugged_in: false,
                saver_on: false,
                minutes_left: Some(50),
            }),
            uptime: Duration::from_secs(9 * 24 * 3600),
        };
        let text = system_summary(&busy);
        assert!(text.contains("Processor: 91% busy"), "{text}");
        assert!(
            text.contains("Memory: 13.0 GB of 14.0 GB in use (93%)"),
            "{text}"
        );
        assert!(
            text.contains("Using the most memory: Chrome (3.0 GB) and Teams (1.0 GB)"),
            "{text}"
        );
        assert!(
            text.contains("Battery: 15%, on battery, about 50 minutes left"),
            "{text}"
        );
        assert!(
            text.contains("Closing Chrome would free the most"),
            "{text}"
        );
        assert!(text.contains("mostly with Chrome"), "{text}");
        assert!(
            text.contains("A restart often clears up slowness"),
            "{text}"
        );
        assert!(text.contains("plug in soon"), "{text}");

        let calm = Snapshot {
            cpu_percent: 7.0,
            memory_used: 4 * gb,
            memory_total: 16 * gb,
            apps: vec![],
            battery: None,
            uptime: Duration::from_secs(3600),
        };
        let text = system_summary(&calm);
        assert!(text.ends_with("Nothing looks wrong right now."), "{text}");
        assert!(!text.contains("Battery"), "{text}");
    }
}
