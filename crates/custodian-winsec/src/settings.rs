//! Named Windows settings Custodian can read, change and put back.
//!
//! A setting is a name, the values it accepts and where Windows keeps it.
//! Nothing outside this list can be changed: the language model (and the
//! person) choose a setting id and one of its own values, and both are checked
//! here before the registry is touched. That is what makes "change any
//! setting" broad without becoming a shell.
//!
//! Every change is planned before it happens ([`Setting::plan`] returns the
//! old value with it), so Cubby can show the person exactly what will change,
//! and [`revert`] can put back what was there — including the case where the
//! value did not exist at all before.
//!
//! Only `HKEY_CURRENT_USER` settings belong here: they need no administrator
//! and affect one person's own desktop. Machine-wide settings are a separate,
//! later decision.

use crate::registry::{self, Hive};
use serde::{Deserialize, Serialize};

/// What went wrong, in words the person can act on.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SettingsError {
    #[error("I don't know a setting called \"{0}\".")]
    UnknownSetting(String),
    #[error("\"{key}\" is not a value for {label}. Choose one of: {options}.")]
    UnknownChoice {
        label: String,
        key: String,
        options: String,
    },
    #[error("I couldn't read {label} from Windows: {reason}.")]
    Read { label: String, reason: String },
    #[error("I couldn't change {label}: {reason}.")]
    Write { label: String, reason: String },
}

/// The topic a setting belongs to, for listing them in groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Appearance,
    Taskbar,
    Files,
}

impl Category {
    /// The word the person sees.
    pub fn label(self) -> &'static str {
        match self {
            Category::Appearance => "appearance",
            Category::Taskbar => "taskbar",
            Category::Files => "file explorer",
        }
    }
}

/// One value a setting may take: `key` is the short word said out loud
/// ("dark", "on", "left"), `stored` is the number Windows keeps for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub key: &'static str,
    pub stored: u32,
}

/// Where a setting is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A 32-bit number under `HKEY_CURRENT_USER`.
    UserDword {
        path: &'static str,
        name: &'static str,
    },
}

/// A setting Custodian may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setting {
    /// Stable id, the only name the model may use.
    pub id: &'static str,
    /// How the setting is named to the person.
    pub label: &'static str,
    pub category: Category,
    /// What changes for the person once it is applied.
    pub effect: &'static str,
    /// When the change shows up (some Windows settings need a re-open).
    pub applies: &'static str,
    pub choices: &'static [Choice],
    pub target: Target,
}

/// Every setting Custodian can read and change.
///
/// These are per-user settings only, each documented by Microsoft and each
/// reversible by writing the previous value back. Adding one is a deliberate
/// act: it needs a documented location, a value for every choice, and a
/// revert that a test proves.
pub const SETTINGS: &[Setting] = &[
    Setting {
        id: "dark_mode_apps",
        label: "Dark mode for apps",
        category: Category::Appearance,
        effect: "Apps and the Start menu use dark colours.",
        applies: "It shows up as soon as an app redraws.",
        choices: &[
            Choice {
                key: "light",
                stored: 1,
            },
            Choice {
                key: "dark",
                stored: 0,
            },
        ],
        target: Target::UserDword {
            path: r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            name: "AppsUseLightTheme",
        },
    },
    Setting {
        id: "dark_mode_windows",
        label: "Dark mode for Windows",
        category: Category::Appearance,
        effect: "The taskbar, Start and Windows' own windows use dark colours.",
        applies: "It shows up right away.",
        choices: &[
            Choice {
                key: "light",
                stored: 1,
            },
            Choice {
                key: "dark",
                stored: 0,
            },
        ],
        target: Target::UserDword {
            path: r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            name: "SystemUsesLightTheme",
        },
    },
    Setting {
        id: "transparency_effects",
        label: "Transparency effects",
        category: Category::Appearance,
        effect: "Windows menus and the taskbar become see-through.",
        applies: "It shows up right away.",
        choices: &[
            Choice {
                key: "on",
                stored: 1,
            },
            Choice {
                key: "off",
                stored: 0,
            },
        ],
        target: Target::UserDword {
            path: r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            name: "EnableTransparency",
        },
    },
    Setting {
        id: "show_hidden_files",
        label: "Show hidden files",
        category: Category::Files,
        effect: "Files Windows normally hides become visible in File Explorer.",
        applies: "It shows up in the next File Explorer window you open.",
        choices: &[
            Choice {
                key: "on",
                stored: 1,
            },
            Choice {
                key: "off",
                stored: 2,
            },
        ],
        target: Target::UserDword {
            path: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            name: "Hidden",
        },
    },
    Setting {
        id: "show_file_extensions",
        label: "Show file name extensions",
        category: Category::Files,
        effect: "File Explorer shows the whole name, so a report reads report.pdf.",
        applies: "It shows up in the next File Explorer window you open.",
        choices: &[
            Choice {
                key: "on",
                stored: 0,
            },
            Choice {
                key: "off",
                stored: 1,
            },
        ],
        target: Target::UserDword {
            path: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            name: "HideFileExt",
        },
    },
    Setting {
        id: "taskbar_alignment",
        label: "Taskbar alignment",
        category: Category::Taskbar,
        effect: "The Start button sits on the left edge or in the middle.",
        applies: "It shows up right away.",
        choices: &[
            Choice {
                key: "left",
                stored: 0,
            },
            Choice {
                key: "center",
                stored: 1,
            },
        ],
        target: Target::UserDword {
            path: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            name: "TaskbarAl",
        },
    },
];

impl Setting {
    fn location(&self) -> (Hive, &'static str, &'static str) {
        match self.target {
            Target::UserDword { path, name } => (Hive::CurrentUser, path, name),
        }
    }

    /// The choice stored as `value`, if one is.
    pub fn choice_for(&self, value: u32) -> Option<Choice> {
        self.choices.iter().copied().find(|c| c.stored == value)
    }

    /// The choice named `key`, matched without case.
    pub fn choice(&self, key: &str) -> Option<Choice> {
        let key = key.trim().to_ascii_lowercase();
        self.choices.iter().copied().find(|c| c.key == key)
    }

    /// The choices, as a list of words for a message.
    pub fn options(&self) -> String {
        self.choices
            .iter()
            .map(|c| c.key)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// What the setting is set to now, or `None` when Windows has no value
    /// for it (which is itself a state, restored exactly by [`revert`]).
    pub fn current(&self) -> Option<Choice> {
        let (hive, path, name) = self.location();
        registry::dword_value(hive, path, name).and_then(|value| self.choice_for(value))
    }

    /// Checks `key` and reads the current value, without changing anything.
    pub fn plan(&self, key: &str) -> Result<Change, SettingsError> {
        let choice = self
            .choice(key)
            .ok_or_else(|| SettingsError::UnknownChoice {
                label: self.label.to_string(),
                key: key.trim().to_string(),
                options: self.options(),
            })?;
        Ok(Change {
            id: self.id.to_string(),
            value: choice.key.to_string(),
            from: self.current().map(|c| c.key.to_string()),
        })
    }

    /// Writes `key`. Callers must have planned and had the person approve it.
    pub fn apply(&self, key: &str) -> Result<(), SettingsError> {
        let choice = self
            .choice(key)
            .ok_or_else(|| SettingsError::UnknownChoice {
                label: self.label.to_string(),
                key: key.trim().to_string(),
                options: self.options(),
            })?;
        self.store(choice.stored)
    }

    /// Writes `value` into the registry, or reports why it could not.
    fn store(&self, value: u32) -> Result<(), SettingsError> {
        let (hive, path, name) = self.location();
        registry::set_dword(hive, path, name, value).map_err(|e| SettingsError::Write {
            label: self.label.to_string(),
            reason: e.to_string(),
        })
    }
}

/// Every setting, in catalogue order.
pub fn all() -> &'static [Setting] {
    SETTINGS
}

/// The setting with this id, matched without case.
pub fn find(id: &str) -> Option<&'static Setting> {
    let id = id.trim().to_ascii_lowercase();
    SETTINGS.iter().find(|s| s.id == id)
}

/// The words in `text`, lower-cased, with anything that is not a letter or a
/// digit treated as a separator. Whole words only: "files" must not match
/// "file", or every setting mentioning files would answer for file names.
fn words_of(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// How well a setting answers the words in `needle`: a word in the label is
/// worth more than one that only appears in the id, because the label is what
/// the person sees and says.
fn score(setting: &Setting, words: &[String]) -> u32 {
    let label = words_of(setting.label);
    let id = words_of(&setting.id.replace('_', " "));
    words
        .iter()
        .map(|w| {
            if label.contains(w) {
                2
            } else if id.contains(w) {
                1
            } else {
                0
            }
        })
        .sum()
}

/// The settings that answer the words in `needle` at all, best first. Used to
/// turn "the taskbar one" into a short list for the person to choose from.
pub fn search(needle: &str) -> Vec<&'static Setting> {
    ranked(needle).into_iter().map(|(s, _)| s).collect()
}

/// As [`search`], with each setting's score.
pub fn ranked(needle: &str) -> Vec<(&'static Setting, u32)> {
    let words: Vec<String> = words_of(needle)
        .into_iter()
        .filter(|w| w.len() >= 3)
        .collect();
    let mut found: Vec<(&'static Setting, u32)> = SETTINGS
        .iter()
        .map(|s| (s, score(s, &words)))
        .filter(|(_, score)| *score > 0)
        .collect();
    // Stable, so settings that score equally keep the order of the catalogue.
    found.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    found
}

/// The one setting the words clearly name, or `None` when nothing matches or
/// two settings match equally well ("dark mode" fits both of them, so the
/// caller must ask rather than choose for the person).
pub fn best(needle: &str) -> Option<&'static Setting> {
    let mut ranked = ranked(needle);
    ranked.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    match ranked.as_slice() {
        [(setting, top), rest @ ..] => rest
            .first()
            .is_none_or(|(_, next)| top > next)
            .then_some(*setting),
        [] => None,
    }
}

/// A setting the person (or the model, with their click) is changing.
///
/// `from` is `None` when Windows had no value for the setting, so a revert
/// removes it again rather than writing a number that was never there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub id: String,
    pub value: String,
    pub from: Option<String>,
}

impl Change {
    /// The setting being changed, if it still exists in this build.
    pub fn setting(&self) -> Option<&'static Setting> {
        find(&self.id)
    }

    /// True when the setting is already at `value`, so there is nothing to do.
    pub fn is_noop(&self) -> bool {
        self.from.as_deref() == Some(self.value.as_str())
    }
}

/// Puts back what was there before `change`: the old value, or removed
/// entirely when Windows had none.
pub fn revert(change: &Change) -> Result<(), SettingsError> {
    let setting =
        find(&change.id).ok_or_else(|| SettingsError::UnknownSetting(change.id.clone()))?;
    match change.from.as_deref() {
        Some(key) => setting.apply(key),
        None => {
            let (hive, path, name) = setting.location();
            registry::remove_value(hive, path, name).map_err(|e| SettingsError::Write {
                label: setting.label.to_string(),
                reason: e.to_string(),
            })
        }
    }
}

/// One line describing a change, for the card the person approves: what the
/// setting becomes, what that does, when it shows up, and that it can be put
/// back.
pub fn describe(change: &Change) -> String {
    let Some(setting) = change.setting() else {
        return format!("Change the setting \"{}\".", change.id);
    };
    let from = change.from.as_deref().unwrap_or("nothing");
    // Saying "nothing" would be confusing on its own: say why it is nothing.
    let note = if change.from.is_none() {
        " (Windows had no value for it)"
    } else {
        ""
    };
    format!(
        "{}: {from} → {}{note}. {} {}\nYou can put this back afterwards.",
        setting.label, change.value, setting.effect, setting.applies
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_is_complete_and_revertible() {
        let mut ids = Vec::new();
        for setting in SETTINGS {
            assert!(!setting.id.is_empty());
            assert!(!setting.label.is_empty());
            assert!(!setting.effect.is_empty());
            assert!(!setting.applies.is_empty());
            assert!(
                !setting.choices.is_empty(),
                "{} has no values to choose",
                setting.id
            );
            assert!(
                !ids.contains(&setting.id),
                "two settings share the id {}",
                setting.id
            );
            ids.push(setting.id);

            // An id must be usable as a lookup key and by the model.
            assert_eq!(find(setting.id).map(|s| s.id), Some(setting.id));
            assert_eq!(
                find(&setting.id.to_ascii_uppercase()).map(|s| s.id),
                Some(setting.id)
            );

            // No two choices may share a key (one word, one meaning) or a
            // stored number (or reading the current value would be ambiguous,
            // and a revert could pick the wrong one).
            let mut keys = Vec::new();
            let mut stored = Vec::new();
            for choice in setting.choices {
                assert!(!choice.key.is_empty(), "{}", setting.id);
                assert_eq!(
                    choice.key,
                    choice.key.to_ascii_lowercase(),
                    "{}",
                    setting.id
                );
                assert!(
                    !keys.contains(&choice.key),
                    "{}: {}",
                    setting.id,
                    choice.key
                );
                assert!(
                    !stored.contains(&choice.stored),
                    "{}: two choices both store {}",
                    setting.id,
                    choice.stored
                );
                keys.push(choice.key);
                stored.push(choice.stored);
                assert_eq!(setting.choice(choice.key), Some(*choice));
                assert_eq!(setting.choice_for(choice.stored), Some(*choice));
            }
            assert_eq!(setting.choice("  no_such_value "), None);
        }
        assert_eq!(find("no_such_setting"), None);
    }

    #[test]
    fn search_finds_settings_by_the_words_people_use() {
        let ids = |words: &str| {
            search(words)
                .iter()
                .map(|s| s.id)
                .collect::<Vec<&'static str>>()
        };
        assert_eq!(
            ids("dark mode"),
            vec!["dark_mode_apps", "dark_mode_windows"]
        );
        assert_eq!(
            ids("make it dark"),
            vec!["dark_mode_apps", "dark_mode_windows"]
        );
        assert_eq!(ids("file extensions"), vec!["show_file_extensions"]);
        assert_eq!(ids("hidden files"), vec!["show_hidden_files"]);
        assert_eq!(ids("taskbar alignment"), vec!["taskbar_alignment"]);
        // A label word ranks above a word that only appears in an id.
        assert_eq!(ids("transparency effects"), vec!["transparency_effects"]);
        // Two words of one setting beat one word of two settings.
        assert_eq!(
            ids("windows dark"),
            vec!["dark_mode_windows", "dark_mode_apps"]
        );
        // Nothing close: the caller must ask, not guess.
        assert!(ids("wireless printer").is_empty());
        // Very short words are ignored rather than matching everything.
        assert!(ids("a an it").is_empty());
    }

    #[test]
    fn the_best_setting_is_only_called_out_when_it_is_clearly_the_one() {
        let best_id = |words: &str| best(words).map(|s| s.id);
        // A clear request names one setting.
        assert_eq!(best_id("windows dark"), Some("dark_mode_windows"));
        assert_eq!(best_id("file extensions"), Some("show_file_extensions"));
        assert_eq!(best_id("hidden files"), Some("show_hidden_files"));
        assert_eq!(best_id("taskbar alignment"), Some("taskbar_alignment"));
        // "dark mode" fits both dark settings equally: ask, do not choose.
        assert_eq!(best_id("dark mode"), None);
        assert_eq!(best_id("make it dark"), None);
        assert_eq!(best_id("wireless printer"), None);
        assert_eq!(best_id("a an it"), None);
    }

    #[test]
    fn planning_checks_the_value_and_reads_the_current_one() {
        let setting = find("show_file_extensions").unwrap();
        // An unknown value is refused with the ones that exist.
        let err = setting.plan("sortof").unwrap_err();
        assert_eq!(
            err,
            SettingsError::UnknownChoice {
                label: "Show file name extensions".into(),
                key: "sortof".into(),
                options: "on, off".into(),
            }
        );
        // Planning never writes: the plan only describes the change.
        let planned = setting.plan(" ON ").unwrap();
        assert_eq!(planned.id, "show_file_extensions");
        assert_eq!(planned.value, "on");
        assert_eq!(planned.from, setting.current().map(|c| c.key.to_string()));
    }

    #[test]
    fn describing_a_change_says_what_it_does_and_that_it_can_be_undone() {
        let change = Change {
            id: "taskbar_alignment".into(),
            value: "left".into(),
            from: Some("center".into()),
        };
        let text = describe(&change);
        assert!(text.contains("Taskbar alignment"), "{text}");
        assert!(text.contains("center → left"), "{text}");
        assert!(text.contains("put this back"), "{text}");

        let fresh = Change {
            id: "transparency_effects".into(),
            value: "off".into(),
            from: None,
        };
        let text = describe(&fresh);
        assert!(text.contains("nothing → off"), "{text}");
        assert!(text.contains("had no value for it"), "{text}");
        assert!(Change {
            id: "gone".into(),
            value: "on".into(),
            from: None
        }
        .setting()
        .is_none());
    }

    #[test]
    fn a_change_to_the_same_value_needs_no_work() {
        let change = Change {
            id: "dark_mode_windows".into(),
            value: "dark".into(),
            from: Some("dark".into()),
        };
        assert!(change.is_noop());
        assert!(!Change {
            id: "dark_mode_windows".into(),
            value: "dark".into(),
            from: Some("light".into())
        }
        .is_noop());
    }

    #[test]
    fn reverting_an_unknown_setting_is_refused() {
        let change = Change {
            id: "no_such_setting".into(),
            value: "on".into(),
            from: Some("off".into()),
        };
        assert_eq!(
            revert(&change),
            Err(SettingsError::UnknownSetting("no_such_setting".into()))
        );
    }

    /// On a real Windows PC the catalogue must be readable: a setting with no
    /// value is allowed (and reverts to "none"), but a name that is declared
    /// must never read back as something the setting does not offer.
    #[cfg(windows)]
    #[test]
    fn reading_real_settings_never_returns_an_undeclared_value() {
        for setting in SETTINGS {
            if let Some(current) = setting.current() {
                assert!(
                    setting.choices.iter().any(|c| c.key == current.key),
                    "{} read back as {}",
                    setting.id,
                    current.key
                );
            }
        }
    }
}
