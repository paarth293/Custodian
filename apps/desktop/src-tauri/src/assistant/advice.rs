//! "Any suggestions?": a quick, read-only check-up of the PC turned into a
//! short list of things worth doing, each with what to say next. Also the
//! rules for Cubby's few unprompted nudges (a nearly full drive, a low
//! battery), which are rate-limited so Cubby never nags (§14).

use super::reply::{bytes, count, duration_words, plural};
use super::system::Snapshot;
use std::time::Duration;

/// What the check-up found. Everything here was measured, not guessed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    /// (drive, free bytes, total bytes) for fixed drives.
    pub drives: Vec<(String, u64, u64)>,
    /// Old temporary files: (count, bytes).
    pub temp: Option<(usize, u64)>,
    /// Installers in Downloads older than a month: (count, bytes).
    pub old_installers: Option<(usize, u64)>,
    /// Recycle Bin: (items, bytes).
    pub recycle_bin: Option<(u64, u64)>,
    /// Copies waiting in the safe place: (count, bytes).
    pub safe_place: (usize, u64),
    pub system: Option<Snapshot>,
    /// Reminders due later today: (task, "5:00 PM").
    pub due_today: Vec<(String, String)>,
}

const GB: u64 = 1024 * 1024 * 1024;
const MB: u64 = 1024 * 1024;

/// A drive is "nearly full" below 10% free or 10 GB free.
pub fn nearly_full(free: u64, total: u64) -> bool {
    total > 0 && (free * 10 < total || free < 10 * GB)
}

/// One suggestion, in plain words, with what to say to act on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub text: String,
    pub try_saying: Option<&'static str>,
}

fn suggest(text: String, try_saying: Option<&'static str>) -> Suggestion {
    Suggestion { text, try_saying }
}

/// The suggestions for `facts`, most useful first.
pub fn advise(facts: &Facts) -> Vec<Suggestion> {
    let mut out = Vec::new();
    for (drive, free, total) in &facts.drives {
        if nearly_full(*free, *total) {
            let used = (total - free) as f64 / *total as f64 * 100.0;
            out.push(suggest(
                format!(
                    "Your {drive} drive is nearly full: {} free ({used:.0}% used).",
                    bytes(*free)
                ),
                Some("biggest files"),
            ));
        }
    }
    if let Some((n, size)) = facts.temp.filter(|(_, size)| *size >= 100 * MB) {
        out.push(suggest(
            format!(
                "{} of old temporary files ({} {}) can go.",
                bytes(size),
                count(n),
                plural(n, "file", "files")
            ),
            Some("clean temp files"),
        ));
    }
    if facts.safe_place.1 >= 100 * MB {
        out.push(suggest(
            format!(
                "{} of duplicate copies {} waiting in my safe place.",
                bytes(facts.safe_place.1),
                plural(facts.safe_place.0, "is", "are")
            ),
            Some("empty the safe place"),
        ));
    }
    if let Some((n, size)) = facts.old_installers.filter(|(_, size)| *size >= 100 * MB) {
        out.push(suggest(
            format!(
                "Your Downloads folder has {} {} older than a month ({}). Once an app is installed, its installer usually isn't needed.",
                count(n),
                plural(n, "installer", "installers"),
                bytes(size)
            ),
            Some("biggest files in Downloads"),
        ));
    }
    if let Some((items, size)) = facts.recycle_bin.filter(|(_, size)| *size >= 500 * MB) {
        out.push(suggest(
            format!(
                "Your Recycle Bin holds {} ({} {}). Emptying it frees that space, but those files are then gone for good, so it's your call.",
                bytes(size),
                count(items as usize),
                plural(items as usize, "item", "items")
            ),
            None,
        ));
    }
    if let Some(s) = &facts.system {
        if s.memory_percent() >= 85.0 {
            if let Some(top) = s.apps.first() {
                out.push(suggest(
                    format!(
                        "Memory is nearly full. {} is using the most ({}).",
                        top.name,
                        bytes(top.memory_bytes)
                    ),
                    Some("is my PC slow?"),
                ));
            }
        }
        if s.uptime >= Duration::from_secs(7 * 24 * 3600) {
            out.push(suggest(
                format!(
                    "Your PC has been on for {}. A restart often speeds things up.",
                    duration_words(s.uptime)
                ),
                None,
            ));
        }
        if let Some(b) = &s.battery {
            if !b.plugged_in && b.percent <= 20 {
                out.push(suggest(
                    format!("The battery is at {}%, so plug in soon.", b.percent),
                    None,
                ));
            }
        }
    }
    for (task, time) in &facts.due_today {
        out.push(suggest(format!("Coming up today at {time}: {task}."), None));
    }
    out
}

/// The answer to "any suggestions?".
pub fn advice_text(suggestions: &[Suggestion]) -> String {
    if suggestions.is_empty() {
        return "Everything looks good! Your drives have room and there's nothing to tidy right now.".into();
    }
    let mut text = String::from("Here's what I'd suggest:");
    for (i, s) in suggestions.iter().enumerate() {
        text.push_str(&format!("\n{}. {}", i + 1, s.text));
        if let Some(say) = s.try_saying {
            text.push_str(&format!(" Say \"{say}\"."));
        }
    }
    text
}

/// What a nudge is about; each kind waits its own time before repeating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NudgeKind {
    DriveFull,
    BatteryLow,
}

impl NudgeKind {
    /// How long before the same kind of nudge may come again.
    pub fn quiet_for(self) -> Duration {
        match self {
            NudgeKind::DriveFull => Duration::from_secs(24 * 3600),
            NudgeKind::BatteryLow => Duration::from_secs(60 * 60),
        }
    }
}

/// The nudge worth giving now, if any: a nearly full drive first, then a
/// low battery. Never more than this one.
pub fn nudge(
    drives: &[(String, u64, u64)],
    snapshot_battery: Option<(u8, bool)>,
) -> Option<(NudgeKind, String)> {
    if let Some((drive, free, _)) = drives
        .iter()
        .find(|(_, free, total)| nearly_full(*free, *total) && *free < 5 * GB)
    {
        return Some((
            NudgeKind::DriveFull,
            format!(
                "Your {drive} drive is almost full ({} left). Say \"suggestions\" and I'll find things to free up.",
                bytes(*free)
            ),
        ));
    }
    if let Some((percent, plugged_in)) = snapshot_battery {
        if !plugged_in && percent <= 15 {
            return Some((
                NudgeKind::BatteryLow,
                format!("Battery at {percent}%. Plug in soon."),
            ));
        }
    }
    None
}

/// Program installers that are usually not needed once the app is installed.
const INSTALLER_EXTENSIONS: &[&str] = &["exe", "msi", "msix", "msixbundle", "appx", "appxbundle"];

/// Installers directly in `dir` not changed for a month: (count, bytes).
pub fn old_installers(dir: &std::path::Path, now: std::time::SystemTime) -> (usize, u64) {
    let month = Duration::from_secs(30 * 24 * 3600);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    entries
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let ext = e.path().extension()?.to_string_lossy().to_lowercase();
            let old = meta
                .modified()
                .ok()
                .and_then(|m| now.duration_since(m).ok())
                .is_some_and(|age| age >= month);
            (meta.is_file() && old && INSTALLER_EXTENSIONS.contains(&ext.as_str()))
                .then_some(meta.len())
        })
        .fold((0, 0), |(n, total), size| (n + 1, total + size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::system::AppUsage;

    #[test]
    fn counts_only_old_installers() {
        let dir = tempfile::tempdir().unwrap();
        let now = std::time::SystemTime::now();
        let write = |name: &str, days: u64| {
            let path = dir.path().join(name);
            std::fs::write(&path, vec![0u8; 100]).unwrap();
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(now - Duration::from_secs(days * 24 * 3600))
                .unwrap();
        };
        write("old-setup.exe", 60);
        write("Tool.MSI", 45);
        write("new-setup.exe", 2);
        write("thesis.docx", 400);
        assert_eq!(old_installers(dir.path(), now), (2, 200));
        assert_eq!(old_installers(&dir.path().join("missing"), now), (0, 0));
    }
    use custodian_winsec::power::Battery;

    fn calm_pc() -> Snapshot {
        Snapshot {
            cpu_percent: 5.0,
            memory_used: 4 * GB,
            memory_total: 16 * GB,
            apps: vec![],
            battery: None,
            uptime: Duration::from_secs(3600),
        }
    }

    #[test]
    fn a_tidy_pc_gets_good_news() {
        let facts = Facts {
            drives: vec![("C:\\".into(), 200 * GB, 500 * GB)],
            system: Some(calm_pc()),
            ..Default::default()
        };
        let advice = advise(&facts);
        assert!(advice.is_empty());
        assert!(advice_text(&advice).starts_with("Everything looks good!"));
    }

    #[test]
    fn suggests_the_most_useful_things_first_with_what_to_say() {
        let mut busy = calm_pc();
        busy.memory_used = 15 * GB;
        busy.apps = vec![AppUsage {
            name: "Chrome".into(),
            memory_bytes: 5 * GB,
            cpu_percent: 10.0,
        }];
        busy.uptime = Duration::from_secs(9 * 24 * 3600);
        busy.battery = Some(Battery {
            percent: 12,
            plugged_in: false,
            saver_on: false,
            minutes_left: None,
        });
        let facts = Facts {
            drives: vec![
                ("C:\\".into(), 8 * GB, 256 * GB),
                ("D:\\".into(), 400 * GB, 900 * GB),
            ],
            temp: Some((900, 2 * GB)),
            old_installers: Some((7, 3 * GB)),
            recycle_bin: Some((40, GB)),
            safe_place: (12, 500 * MB),
            system: Some(busy),
            due_today: vec![("Call mom".into(), "5:00 PM".into())],
        };
        let text = advice_text(&advise(&facts));
        assert!(text.starts_with("Here's what I'd suggest:\n1. Your C:\\ drive is nearly full: 8.0 GB free (97% used). Say \"biggest files\"."), "{text}");
        assert!(!text.contains("D:\\"), "{text}");
        assert!(
            text.contains(
                "2.0 GB of old temporary files (900 files) can go. Say \"clean temp files\"."
            ),
            "{text}"
        );
        assert!(text.contains("500 MB of duplicate copies are waiting in my safe place. Say \"empty the safe place\"."), "{text}");
        assert!(
            text.contains("7 installers older than a month (3.0 GB)"),
            "{text}"
        );
        assert!(
            text.contains("Recycle Bin holds 1.0 GB (40 items)"),
            "{text}"
        );
        assert!(
            text.contains("Memory is nearly full. Chrome is using the most (5.0 GB)."),
            "{text}"
        );
        assert!(text.contains("on for 9 days"), "{text}");
        assert!(text.contains("battery is at 12%"), "{text}");
        assert!(
            text.contains("Coming up today at 5:00 PM: Call mom."),
            "{text}"
        );
    }

    #[test]
    fn small_amounts_are_not_worth_mentioning() {
        let facts = Facts {
            temp: Some((3, 20 * MB)),
            old_installers: Some((1, 50 * MB)),
            recycle_bin: Some((2, 100 * MB)),
            safe_place: (1, MB),
            ..Default::default()
        };
        assert!(advise(&facts).is_empty());
    }

    #[test]
    fn nudges_only_for_a_drive_about_to_fill_or_a_dying_battery() {
        let roomy = vec![("C:\\".into(), 50 * GB, 256 * GB)];
        assert_eq!(nudge(&roomy, Some((80, false))), None);
        // Nearly full by percentage but still 9 GB free: a suggestion, not a nudge.
        assert_eq!(nudge(&[("C:\\".into(), 9 * GB, 256 * GB)], None), None);
        let full = vec![("C:\\".into(), 3 * GB, 256 * GB)];
        let (kind, text) = nudge(&full, Some((10, false))).unwrap();
        assert_eq!(kind, NudgeKind::DriveFull, "the drive comes first");
        assert!(text.contains("almost full (3.0 GB left)"), "{text}");
        let (kind, text) = nudge(&roomy, Some((10, false))).unwrap();
        assert_eq!(kind, NudgeKind::BatteryLow);
        assert_eq!(text, "Battery at 10%. Plug in soon.");
        assert_eq!(nudge(&roomy, Some((10, true))), None, "charging");
        assert!(NudgeKind::DriveFull.quiet_for() > NudgeKind::BatteryLow.quiet_for());
    }
}
