//! Changing Windows settings, and putting them back.
//!
//! The settings themselves are in `custodian_winsec::settings`; the words
//! people use for their values live here, next to the sealed record of what
//! was changed. Keeping that record means "put my theme back" still works
//! after a restart, and it is sealed with DPAPI for the same reason the to-do
//! list is: it says something about the person's own PC (D-05, SR-F2).

use super::reply::{count, plural, safe_name};
use custodian_winsec::settings::{self, Change, Setting};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Most changes remembered, so the sealed file cannot grow without end.
const MAX_HISTORY: usize = 200;
/// Reverted changes are forgotten after this long.
const KEEP_REVERTED_SECS: i64 = 90 * 24 * 60 * 60;

/// A word the person uses, and the value it means. Matched in the order the
/// words appear in the request, so "show hidden files" reads "show" (on)
/// before "hidden", which is part of the setting's own name.
const VALUE_WORDS: &[(&str, &str)] = &[
    ("show", "on"),
    ("on", "on"),
    ("enable", "on"),
    ("enabled", "on"),
    ("off", "off"),
    ("disable", "off"),
    ("disabled", "off"),
    ("hide", "off"),
    ("dark", "dark"),
    ("night", "dark"),
    ("light", "light"),
    ("day", "light"),
    ("left", "left"),
    ("center", "center"),
    ("centre", "center"),
    ("middle", "center"),
];

/// What a request about settings came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// A setting and the value asked for.
    Ready(&'static Setting, String),
    /// The setting was named but not the value.
    NeedsValue(&'static Setting),
    /// More than one setting fits; the caller must ask which.
    Ambiguous(Vec<&'static Setting>),
    /// Nothing in the catalogue fits.
    Unknown,
}

/// Resolves words like "make windows dark" or "show file extensions" to a
/// setting and one of its values. Never guesses between two settings.
pub fn resolve(words: &str) -> Resolution {
    let Some(setting) = settings::best(words) else {
        let found = settings::search(words);
        return if found.is_empty() {
            Resolution::Unknown
        } else {
            Resolution::Ambiguous(found)
        };
    };
    match value_for(setting, words) {
        Some(value) => Resolution::Ready(setting, value),
        None => Resolution::NeedsValue(setting),
    }
}

/// The value `words` asks for, read in the order the person said them, or
/// `None` when the request does not say. A word the setting has no value for
/// is passed over, so "turn on dark mode" reaches "dark".
fn value_for(setting: &Setting, words: &str) -> Option<String> {
    let lower = words.to_ascii_lowercase();
    // A light/dark setting has no on or off of its own: turning it off means
    // light, and turning it on means dark. Checked as each word comes, so
    // "turn off dark mode" is light even though "dark" is said later.
    let light_dark = setting.choice("light").is_some() && setting.choice("dark").is_some();
    for word in lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let Some((_, value)) = VALUE_WORDS.iter().find(|(w, _)| *w == word) else {
            continue;
        };
        if setting.choice(value).is_some() {
            return Some((*value).to_string());
        }
        if light_dark {
            match *value {
                "off" => return Some("light".into()),
                "on" => return Some("dark".into()),
                _ => {}
            }
        }
    }
    None
}

/// A change Cubby made, kept so it can be put back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    pub change: Change,
    pub applied_unix: i64,
    #[serde(default)]
    pub reverted: bool,
}

/// Every change Cubby made, newest last.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsHistory {
    next_id: u64,
    entries: Vec<Entry>,
}

impl SettingsHistory {
    /// Records a change that was just applied.
    pub fn record(&mut self, change: Change, now_unix: i64) -> Entry {
        self.next_id += 1;
        let entry = Entry {
            id: self.next_id,
            change,
            applied_unix: now_unix,
            reverted: false,
        };
        self.entries.push(entry.clone());
        entry
    }

    /// The most recent change that has not been put back.
    pub fn last_open(&self) -> Option<&Entry> {
        self.entries.iter().rev().find(|e| !e.reverted)
    }

    /// Marks a change as put back; false when it was already.
    pub fn mark_reverted(&mut self, id: u64) -> bool {
        match self.entries.iter_mut().find(|e| e.id == id && !e.reverted) {
            Some(entry) => {
                entry.reverted = true;
                true
            }
            None => false,
        }
    }

    /// Keeps the newest changes, and any older one not yet put back (F24 / A40).
    fn prune(&mut self, now_unix: i64) {
        // Drop old reverted entries older than KEEP_REVERTED_SECS
        self.entries
            .retain(|e| !e.reverted || now_unix - e.applied_unix < KEEP_REVERTED_SECS);
        // If still over MAX_HISTORY, drop oldest reverted entries first to preserve open changes
        if self.entries.len() > MAX_HISTORY {
            let mut to_drop = self.entries.len() - MAX_HISTORY;
            let mut i = 0;
            while i < self.entries.len() && to_drop > 0 {
                if self.entries[i].reverted {
                    self.entries.remove(i);
                    to_drop -= 1;
                } else {
                    i += 1;
                }
            }
        }
    }
}

/// Reads the sealed history at `path`. Missing is empty; one that cannot be
/// unsealed is set aside as `*.unreadable` rather than overwritten.
pub fn load(path: &Path) -> io::Result<SettingsHistory> {
    let sealed = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(SettingsHistory::default()),
        Err(err) => return Err(err),
    };
    let history = custodian_winsec::dpapi::unprotect(&sealed)
        .ok()
        .and_then(|plain| serde_json::from_slice(&plain).ok());
    match history {
        Some(history) => Ok(history),
        None => {
            fs::rename(path, path.with_extension("unreadable"))?;
            Ok(SettingsHistory::default())
        }
    }
}

/// Seals and writes the history, all at once, so a crash leaves no half file.
pub fn save(path: &Path, history: &mut SettingsHistory, now_unix: i64) -> io::Result<()> {
    history.prune(now_unix);
    let plain = serde_json::to_vec(history).map_err(io::Error::other)?;
    let sealed = custodian_winsec::dpapi::protect(&plain)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(&sealed)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

/// What Cubby can change, with what this PC has them set to now.
pub fn list_text() -> String {
    let mut text =
        "Here's what I can change, and how it is set on this PC now. Ask me for any of them."
            .to_string();
    let mut category = None;
    for setting in settings::all() {
        if category != Some(setting.category) {
            category = Some(setting.category);
            text.push_str(&format!("\n\n{}:", setting.category.label()));
        }
        let now = match setting.current() {
            Some(choice) => choice.key.to_string(),
            None => "not set".to_string(),
        };
        text.push_str(&format!(
            "\n• {} — now {now} (say {}).",
            setting.label,
            setting.options()
        ));
    }
    text
}

/// After a change was applied.
pub fn changed_text(change: &Change) -> String {
    let Some(setting) = change.setting() else {
        return "Done — the setting changed.".into();
    };
    format!(
        "Done — {} is now {}. {} Say \"put the last setting back\" if you'd rather not keep it.",
        setting.label, change.value, setting.applies
    )
}

/// When the setting was already at that value.
pub fn already_text(change: &Change) -> String {
    match change.setting() {
        Some(setting) => format!("{} was already {}.", setting.label, change.value),
        None => "That setting was already like that.".into(),
    }
}

/// After a change was put back.
pub fn reverted_text(change: &Change) -> String {
    let Some(setting) = change.setting() else {
        return "Put it back the way it was.".into();
    };
    match change.from.as_deref() {
        Some(from) => format!("Put {} back to {from}.", setting.label),
        None => format!(
            "{} is back to how Windows came: I removed the value I added.",
            setting.label
        ),
    }
}

/// When there is nothing to put back.
pub const NOTHING_TO_REVERT: &str =
    "I haven't changed a setting recently, so there's nothing to put back.";

/// When the request names a setting but not a value.
pub fn needs_value_text(setting: &Setting) -> String {
    format!(
        "Should {} be {}?",
        setting.label,
        setting
            .choices
            .iter()
            .map(|c| c.key)
            .collect::<Vec<_>>()
            .join(" or ")
    )
}

/// When more than one setting fits the request.
pub fn which_text(found: &[&Setting]) -> String {
    let names: Vec<String> = found
        .iter()
        .take(5)
        .map(|s| format!("{} (say {})", s.label, s.id))
        .collect();
    format!("Which one do you mean? {}", names.join(", or "))
}

/// When nothing in the catalogue fits.
pub fn unknown_text(words: &str) -> String {
    format!(
        "I don't have a setting called \"{}\". I can only change the settings you see in \"what settings can you change?\" — try asking for one of those, and I'll show you the whole list.",
        safe_name(words)
    )
}

/// "1 setting" / "3 settings".
pub fn count_text(n: usize) -> String {
    format!("{} {}", count(n), plural(n, "setting", "settings"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use custodian_winsec::settings::SettingsError;

    fn ready(words: &str) -> (&'static str, String) {
        match resolve(words) {
            Resolution::Ready(setting, value) => (setting.id, value),
            other => panic!("{words} did not resolve: {other:?}"),
        }
    }

    #[test]
    fn resolves_the_requests_people_actually_make() {
        assert_eq!(
            ready("make windows dark"),
            ("dark_mode_windows", "dark".into())
        );
        assert_eq!(
            ready("turn on dark mode for windows"),
            ("dark_mode_windows", "dark".into())
        );
        assert_eq!(
            ready("show file extensions"),
            ("show_file_extensions", "on".into())
        );
        assert_eq!(
            ready("hide file extensions"),
            ("show_file_extensions", "off".into())
        );
        assert_eq!(
            ready("show hidden files"),
            ("show_hidden_files", "on".into())
        );
        assert_eq!(
            ready("turn off transparency effects"),
            ("transparency_effects", "off".into())
        );
        assert_eq!(
            ready("put my taskbar on the left"),
            ("taskbar_alignment", "left".into())
        );
        assert_eq!(
            ready("move the taskbar to the middle"),
            ("taskbar_alignment", "center".into())
        );
    }

    #[test]
    fn a_light_dark_setting_reads_on_and_off_as_dark_and_light() {
        // "turn off dark mode" is not "on or off": it is light.
        assert_eq!(
            ready("turn off dark mode apps"),
            ("dark_mode_apps", "light".into())
        );
        assert_eq!(
            ready("switch on dark mode apps"),
            ("dark_mode_apps", "dark".into())
        );
    }

    #[test]
    fn asks_instead_of_guessing() {
        // Two settings fit equally well.
        assert!(matches!(resolve("dark mode"), Resolution::Ambiguous(_)));
        match resolve("dark mode") {
            Resolution::Ambiguous(found) => assert_eq!(found.len(), 2),
            other => panic!("{other:?}"),
        }
        // A setting with no value said.
        assert_eq!(
            resolve("file extensions"),
            Resolution::NeedsValue(settings::find("show_file_extensions").unwrap())
        );
        // Nothing at all.
        assert_eq!(resolve("wireless printer"), Resolution::Unknown);
        assert_eq!(resolve(""), Resolution::Unknown);
    }

    /// Junk never becomes a change. It may still match a setting's own words
    /// ("windows" is the OS), but it can only ever end in a question, never
    /// in a value to write: only [`Resolution::Ready`] applies anything.
    #[test]
    fn junk_never_becomes_a_change() {
        for words in ["C:\\Windows", "rm -rf /", "drop table settings", ""] {
            assert!(
                !matches!(resolve(words), Resolution::Ready(..)),
                "{words} resolved to a change"
            );
        }
    }

    #[test]
    fn history_remembers_the_last_change_until_it_is_put_back() {
        let mut history = SettingsHistory::default();
        assert!(history.last_open().is_none());
        let first = history.record(
            Change {
                id: "dark_mode_apps".into(),
                value: "dark".into(),
                from: Some("light".into()),
            },
            100,
        );
        let second = history.record(
            Change {
                id: "show_file_extensions".into(),
                value: "on".into(),
                from: None,
            },
            200,
        );
        assert_eq!(history.last_open().unwrap().id, second.id);
        assert!(history.mark_reverted(second.id));
        assert!(!history.mark_reverted(second.id), "only once");
        assert_eq!(
            history.last_open().unwrap().id,
            first.id,
            "the older one is next"
        );
        assert!(!history.mark_reverted(999));
        assert_eq!(history.entries.len(), 2);
    }

    #[test]
    fn history_forgets_old_put_back_changes_and_never_keeps_open_ones() {
        let mut history = SettingsHistory::default();
        let open = history.record(
            Change {
                id: "dark_mode_windows".into(),
                value: "dark".into(),
                from: None,
            },
            0,
        );
        let done = history.record(
            Change {
                id: "transparency_effects".into(),
                value: "off".into(),
                from: Some("on".into()),
            },
            0,
        );
        history.mark_reverted(done.id);
        history.prune(0);
        assert_eq!(history.entries.len(), 2, "nothing old yet");

        history.prune(KEEP_REVERTED_SECS + 1);
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].id, open.id, "the open one stays");
    }

    #[cfg(windows)]
    #[test]
    fn saves_sealed_and_loads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.dpapi");
        assert_eq!(load(&path).unwrap(), SettingsHistory::default());

        let mut original = SettingsHistory::default();
        original.record(
            Change {
                id: "dark_mode_apps".into(),
                value: "light".into(),
                from: Some("dark".into()),
            },
            100,
        );
        save(&path, &mut original, 100).unwrap();
        let raw = fs::read(&path).unwrap();
        assert!(
            !raw.windows(5).any(|w| w == b"light"),
            "sealed, not plain text"
        );
        assert_eq!(load(&path).unwrap(), original);

        // One that cannot be unsealed is set aside, not overwritten.
        fs::write(&path, b"garbage").unwrap();
        assert_eq!(load(&path).unwrap(), SettingsHistory::default());
        assert!(path.with_extension("unreadable").exists());
    }

    #[test]
    fn says_what_happened_in_plain_words() {
        let change = Change {
            id: "show_file_extensions".into(),
            value: "on".into(),
            from: Some("off".into()),
        };
        assert!(changed_text(&change).contains("Show file name extensions is now on"));
        assert!(changed_text(&change).contains("put the last setting back"));
        assert_eq!(
            already_text(&change),
            "Show file name extensions was already on."
        );
        assert!(reverted_text(&change).contains("back to off"));
        let fresh = Change {
            id: "taskbar_alignment".into(),
            value: "left".into(),
            from: None,
        };
        assert!(reverted_text(&fresh).contains("removed the value I added"));

        let text = list_text();
        for setting in settings::all() {
            assert!(text.contains(setting.label), "{} missing", setting.id);
        }
        assert_eq!(count_text(1), "1 setting");
        assert_eq!(count_text(3), "3 settings");
        assert!(unknown_text("screensaver").contains("screensaver"));
        assert!(
            needs_value_text(settings::find("taskbar_alignment").unwrap())
                .contains("left or center")
        );
    }

    #[test]
    fn a_refused_value_is_reported_by_the_setting_itself() {
        let setting = settings::find("taskbar_alignment").unwrap();
        assert_eq!(
            setting.apply("sideways"),
            Err(SettingsError::UnknownChoice {
                label: "Taskbar alignment".into(),
                key: "sideways".into(),
                options: "left, center".into(),
            })
        );
    }

    #[test]
    fn prune_preserves_unreverted_changes_even_over_limit() {
        let mut history = SettingsHistory::default();
        // Add 250 open (unreverted) changes
        for i in 0..250 {
            history.record(
                Change {
                    id: "dark_mode_apps".into(),
                    value: "light".into(),
                    from: Some("dark".into()),
                },
                i,
            );
        }
        history.prune(1000);
        // All 250 open changes must be preserved since none have been reverted
        assert_eq!(history.entries.len(), 250);

        // Mark first 100 as reverted
        for i in 1..=100 {
            history.mark_reverted(i);
        }
        // Prune now: should drop 50 oldest reverted entries to get down to 200
        history.prune(1000);
        assert_eq!(history.entries.len(), 200);
        // The 150 open changes are all intact
        assert_eq!(history.entries.iter().filter(|e| !e.reverted).count(), 150);
    }
}
