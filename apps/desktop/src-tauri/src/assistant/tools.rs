#![allow(dead_code)]
//! The tools Cubby can run, whether a rule or the language model chose them
//! (mirrors `services/agent/src/tools.ts`, §40).
//!
//! The model only names a tool and fills in arguments; this module checks
//! every argument again before anything happens: folders must be the
//! person's known folders, apps come from the allow-list, websites must look
//! like a plain domain, and free text is short and printable. A tool call
//! that fails these checks is refused and the model is told why.

use super::intent::{self, App, Intent, KnownFolder, Scope};
use super::reply;
use serde_json::Value;

/// Longest free-text argument the model may pass (a search, a file name).
pub const MAX_TEXT_CHARS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCall {
    CheckDiskSpace,
    FindLargeFiles {
        scope: Scope,
        count: usize,
    },
    FindDuplicateFiles(Scope),
    FindFiles {
        name: String,
        scope: Scope,
    },
    SystemStatus,
    OpenApp(App),
    OpenFolder(KnownFolder),
    WebSearch(String),
    OpenWebsite {
        name: String,
        url: String,
    },
    CleanupHistory,
    UndoLastCleanup,
    /// The apps open on the desktop.
    ListOpenApps,
    /// Asks an open app to close, as its X button does.
    CloseApp(String),
    /// Adds a to-do item. `when` is a time phrase from the model ("at
    /// 5pm"); the person's own requests carry the time inside `text`.
    AddTask {
        text: String,
        when: Option<String>,
    },
    ListTasks,
    CompleteTask(String),
    /// Removes a to-do item. Only the person can ask for this.
    RemoveTask(String),
    /// Proposes deleting temporary files older than a week (needs approval).
    CleanTempFiles,
    /// A quick check-up with suggestions.
    Suggestions,
    /// How Cubby protects the person's data, as it stands now.
    SecurityStatus,
    /// Opens an app from the Start menu by name, or else a file by name.
    OpenProgram(String),
    /// Opens one of the person's documents or media files by name.
    OpenFile(String),
    /// Shows the settings Cubby may change and what this PC has them set to.
    ListSettings,
    /// Changes a named Windows setting. The setting and the value are both
    /// checked against the catalogue in `custodian_winsec::settings`, so the
    /// model can only ever ask for one of Cubby's own settings.
    ChangeSetting {
        setting: String,
        value: String,
    },
    /// Puts back the last settings change. Only the person can ask for this:
    /// the model has no name for it and [`ToolCall::model_may_request`] says no.
    RevertSettingChange,
    /// Frees the safe place's space now. Only the person can ask for this:
    /// the model has no name for it, and [`ToolCall::model_may_request`] says no.
    EmptySafePlace,
    /// Moves a file to a new location.
    MoveFile {
        source: String,
        destination: String,
    },
    /// Copies a file to a new location.
    CopyFile {
        source: String,
        destination: String,
    },
    /// Renames a file in place.
    RenameFile {
        path: String,
        new_name: String,
    },
    /// Creates a new directory.
    CreateFolder(String),
    /// Inspects a file or folder and returns its measured properties.
    GetFileInfo(String),
}

impl ToolCall {
    /// The tool for a request the rules understood, if it is one.
    pub fn from_intent(intent: &Intent) -> Option<Self> {
        Some(match intent {
            Intent::DiskSpace => ToolCall::CheckDiskSpace,
            Intent::LargestFiles { scope, count } => ToolCall::FindLargeFiles {
                scope: scope.clone(),
                count: *count,
            },
            Intent::FindDuplicates(scope) => ToolCall::FindDuplicateFiles(scope.clone()),
            Intent::FindFiles { name, scope } => ToolCall::FindFiles {
                name: name.clone(),
                scope: scope.clone(),
            },
            Intent::SystemStatus => ToolCall::SystemStatus,
            Intent::OpenApp(app) => ToolCall::OpenApp(*app),
            Intent::OpenFolder(folder) => ToolCall::OpenFolder(*folder),
            Intent::WebSearch(query) => ToolCall::WebSearch(query.clone()),
            Intent::OpenWebsite { name, url } => ToolCall::OpenWebsite {
                name: name.clone(),
                url: url.clone(),
            },
            Intent::History => ToolCall::CleanupHistory,
            Intent::Undo => ToolCall::UndoLastCleanup,
            Intent::EmptySafePlace => ToolCall::EmptySafePlace,
            Intent::ListOpenApps => ToolCall::ListOpenApps,
            Intent::CloseApp(name) => ToolCall::CloseApp(name.clone()),
            Intent::AddTask(text) => ToolCall::AddTask {
                text: text.clone(),
                when: None,
            },
            Intent::ListTasks => ToolCall::ListTasks,
            Intent::CompleteTask(name) => ToolCall::CompleteTask(name.clone()),
            Intent::RemoveTask(name) => ToolCall::RemoveTask(name.clone()),
            Intent::CleanTempFiles => ToolCall::CleanTempFiles,
            Intent::Suggestions => ToolCall::Suggestions,
            Intent::SecurityStatus => ToolCall::SecurityStatus,
            Intent::OpenUnknown(target) => ToolCall::OpenProgram(target.clone()),
            Intent::ListSettings => ToolCall::ListSettings,
            Intent::RevertSetting => ToolCall::RevertSettingChange,
            Intent::MoveFile {
                source,
                destination,
            } => ToolCall::MoveFile {
                source: source.clone(),
                destination: destination.clone(),
            },
            Intent::CopyFile {
                source,
                destination,
            } => ToolCall::CopyFile {
                source: source.clone(),
                destination: destination.clone(),
            },
            Intent::RenameFile { path, new_name } => ToolCall::RenameFile {
                path: path.clone(),
                new_name: new_name.clone(),
            },
            Intent::CreateFolder(path) => ToolCall::CreateFolder(path.clone()),
            Intent::GetFileInfo(path) => ToolCall::GetFileInfo(path.clone()),
            // `Intent::Settings` carries the person's own words and is resolved
            // against the catalogue before it becomes a call (see `respond`).
            _ => return None,
        })
    }

    /// Whether the language model may ask for this at all. Removing the way
    /// back (emptying the safe place) is the person's decision alone.
    pub fn model_may_request(&self) -> bool {
        !matches!(
            self,
            ToolCall::EmptySafePlace | ToolCall::RemoveTask(_) | ToolCall::RevertSettingChange
        )
    }

    /// Checks a tool call from the language model. The error explains the
    /// problem in a few words, for the model to correct.
    pub fn parse(name: &str, args: &Value) -> Result<Self, String> {
        let args = match args {
            Value::Object(_) => args,
            Value::Null => &Value::Null,
            _ => return Err("the arguments must be an object".into()),
        };
        Ok(match name {
            "check_disk_space" => ToolCall::CheckDiskSpace,
            "find_large_files" => {
                let count = args
                    .get("count")
                    .and_then(Value::as_u64)
                    .map(|n| n.clamp(1, 100) as usize)
                    .unwrap_or(10);
                ToolCall::FindLargeFiles {
                    scope: read_scope_arg(args)?,
                    count,
                }
            }
            "find_duplicate_files" => ToolCall::FindDuplicateFiles(folders_arg(args)?),
            "find_files" => ToolCall::FindFiles {
                name: text_arg(args, "name")?,
                scope: read_scope_arg(args)?,
            },
            "system_status" => ToolCall::SystemStatus,
            "open_app" => {
                let app = text_arg(args, "app")?;
                ToolCall::OpenApp(
                    App::from_tool_name(&app).ok_or_else(|| format!("unknown app \"{app}\""))?,
                )
            }
            "open_folder" => {
                let folder = text_arg(args, "folder")?;
                ToolCall::OpenFolder(
                    KnownFolder::from_label(&folder)
                        .ok_or_else(|| format!("unknown folder \"{folder}\""))?,
                )
            }
            "web_search" => ToolCall::WebSearch(text_arg(args, "query")?),
            "open_website" => {
                let site = text_arg(args, "site")?;
                let (name, url) = intent::website(&site)
                    .ok_or_else(|| format!("\"{site}\" is not a website name or domain"))?;
                ToolCall::OpenWebsite { name, url }
            }
            "cleanup_history" => ToolCall::CleanupHistory,
            "undo_last_cleanup" => ToolCall::UndoLastCleanup,
            "list_open_apps" => ToolCall::ListOpenApps,
            "close_app" => ToolCall::CloseApp(text_arg(args, "name")?),
            "add_task" => ToolCall::AddTask {
                text: text_arg(args, "text")?,
                when: optional_text_arg(args, "when")?,
            },
            "list_tasks" => ToolCall::ListTasks,
            "clean_temp_files" => ToolCall::CleanTempFiles,
            "pc_checkup" => ToolCall::Suggestions,
            "privacy_report" => ToolCall::SecurityStatus,
            "open_installed_app" => ToolCall::OpenProgram(text_arg(args, "name")?),
            "open_file" => ToolCall::OpenFile(text_arg(args, "name")?),
            "complete_task" => ToolCall::CompleteTask(text_arg(args, "text")?),
            "list_settings" => ToolCall::ListSettings,
            "change_setting" => {
                let setting = text_arg(args, "setting")?;
                let value = text_arg(args, "value")?;
                // Both halves are checked against the catalogue here, so a
                // call that names anything else is refused before it can
                // reach a card or the registry.
                let known = custodian_winsec::settings::find(&setting)
                    .ok_or_else(|| format!("there is no setting called \"{setting}\""))?;
                if known.choice(&value).is_none() {
                    return Err(format!(
                        "\"{value}\" is not a value for {}; use one of: {}",
                        known.label,
                        known.options()
                    ));
                }
                ToolCall::ChangeSetting { setting, value }
            }
            "move_file" => ToolCall::MoveFile {
                source: path_arg(args, "source")?,
                destination: path_arg(args, "destination")?,
            },
            "copy_file" => ToolCall::CopyFile {
                source: path_arg(args, "source")?,
                destination: path_arg(args, "destination")?,
            },
            "rename_file" => ToolCall::RenameFile {
                path: path_arg(args, "path")?,
                new_name: text_arg(args, "new_name")?,
            },
            "create_folder" => ToolCall::CreateFolder(path_arg(args, "path")?),
            "get_file_info" => ToolCall::GetFileInfo(path_arg(args, "path")?),
            other => return Err(format!("there is no tool called \"{other}\"")),
        })
    }
}

/// A required, short, printable text argument.
fn text_arg(args: &Value, key: &str) -> Result<String, String> {
    let raw = args
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("\"{key}\" is missing"))?;
    // Line breaks and tabs become spaces; other control and invisible
    // formatting characters (which could make the card shown to the person
    // read differently from what is sent) are dropped.
    let printable: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| !c.is_control() && !reply::is_hidden_format(*c))
        .collect();
    let text = printable.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        Err(format!("\"{key}\" is empty"))
    } else if text.chars().count() > MAX_TEXT_CHARS {
        Err(format!("\"{key}\" is too long"))
    } else {
        Ok(text)
    }
}

/// Paths retain meaningful internal spaces and have their own Windows-sized bound.
fn path_arg(args: &Value, key: &str) -> Result<String, String> {
    let raw = args
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("\"{key}\" is missing"))?;
    let path = raw.trim();
    if path.is_empty()
        || path.encode_utf16().count() > 32767
        || path
            .chars()
            .any(|c| c.is_control() || reply::is_hidden_format(c))
    {
        return Err(format!("\"{key}\" is not a valid printable path"));
    }
    Ok(path.into())
}

/// An optional text argument: absent, null or "" mean none.
fn optional_text_arg(args: &Value, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(_) => text_arg(args, key).map(Some),
    }
}

/// The optional `folders` list: known folder names only. None or an empty
/// list means all of the person's folders.
fn read_scope_arg(args: &Value) -> Result<Scope, String> {
    match folders_arg(args) {
        Ok(Scope::UserFolders) => Ok(Scope::AllDrives),
        Ok(scope) => Ok(scope),
        Err(original) => {
            let items = args
                .get("folders")
                .and_then(Value::as_array)
                .ok_or_else(|| original.clone())?;
            if items.len() != 1 {
                return Err("Use one explicit local path or a list of known folders.".into());
            }
            let label = items[0].as_str().ok_or(original)?;
            if ["all", "all_drives", "system"].contains(&label.to_ascii_lowercase().as_str()) {
                return Ok(Scope::AllDrives);
            }
            let path = if label.len() == 2
                && label.as_bytes()[0].is_ascii_alphabetic()
                && label.ends_with(':')
            {
                std::path::PathBuf::from(format!("{label}\\"))
            } else {
                std::path::PathBuf::from(label)
            };
            if path.is_absolute() && custodian_winsec::fsops::is_local_volume(&path) {
                Ok(Scope::Custom(path))
            } else {
                Err("Search scope must be an absolute path on a local drive.".into())
            }
        }
    }
}

fn folders_arg(args: &Value) -> Result<Scope, String> {
    let items = match args.get("folders") {
        None | Some(Value::Null) => return Ok(Scope::UserFolders),
        Some(Value::Array(items)) => items,
        Some(_) => return Err("\"folders\" must be a list".into()),
    };
    let mut folders = Vec::new();
    for item in items {
        let label = item.as_str().ok_or("\"folders\" must list folder names")?;
        let folder =
            KnownFolder::from_label(label).ok_or_else(|| format!("unknown folder \"{label}\""))?;
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    Ok(if folders.is_empty() {
        Scope::UserFolders
    } else {
        Scope::Folders(folders)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_well_formed_calls() {
        assert_eq!(
            ToolCall::parse("check_disk_space", &json!({})),
            Ok(ToolCall::CheckDiskSpace)
        );
        assert_eq!(
            ToolCall::parse("check_disk_space", &Value::Null),
            Ok(ToolCall::CheckDiskSpace)
        );
        assert_eq!(
            ToolCall::parse(
                "find_large_files",
                &json!({"folders": ["Downloads", "Videos"]})
            ),
            Ok(ToolCall::FindLargeFiles {
                scope: Scope::Folders(vec![KnownFolder::Downloads, KnownFolder::Videos]),
                count: 10,
            })
        );
        assert_eq!(
            ToolCall::parse(
                "find_large_files",
                &json!({"folders": ["Downloads"], "count": 1})
            ),
            Ok(ToolCall::FindLargeFiles {
                scope: Scope::Folders(vec![KnownFolder::Downloads]),
                count: 1,
            })
        );
        assert_eq!(
            ToolCall::parse("find_duplicate_files", &json!({"folders": []})),
            Ok(ToolCall::FindDuplicateFiles(Scope::UserFolders))
        );
        assert_eq!(
            ToolCall::parse("find_files", &json!({"name": "  tax   2025 "})),
            Ok(ToolCall::FindFiles {
                name: "tax 2025".into(),
                scope: Scope::AllDrives
            })
        );
        assert_eq!(
            ToolCall::parse("open_app", &json!({"app": "file_explorer"})),
            Ok(ToolCall::OpenApp(App::FileExplorer))
        );
        assert_eq!(
            ToolCall::parse("open_folder", &json!({"folder": "pictures"})),
            Ok(ToolCall::OpenFolder(KnownFolder::Pictures))
        );
        assert_eq!(
            ToolCall::parse("close_app", &json!({"name": "Notepad"})),
            Ok(ToolCall::CloseApp("Notepad".into()))
        );
        assert_eq!(
            ToolCall::parse("list_open_apps", &json!({})),
            Ok(ToolCall::ListOpenApps)
        );
        assert_eq!(
            ToolCall::parse("add_task", &json!({"text": "Call mom", "when": "at 5pm"})),
            Ok(ToolCall::AddTask {
                text: "Call mom".into(),
                when: Some("at 5pm".into())
            })
        );
        assert_eq!(
            ToolCall::parse("add_task", &json!({"text": "Buy milk", "when": ""})),
            Ok(ToolCall::AddTask {
                text: "Buy milk".into(),
                when: None
            })
        );
        assert!(ToolCall::parse("add_task", &json!({"when": "at 5pm"})).is_err());
        assert!(ToolCall::parse("add_task", &json!({"text": "x", "when": 5})).is_err());
        assert_eq!(
            ToolCall::parse("complete_task", &json!({"text": "milk"})),
            Ok(ToolCall::CompleteTask("milk".into()))
        );
        assert!(ToolCall::parse("remove_task", &json!({"text": "milk"})).is_err());
        assert!(!ToolCall::RemoveTask("milk".into()).model_may_request());
        assert_eq!(
            ToolCall::parse("open_website", &json!({"site": "YouTube"})),
            Ok(ToolCall::OpenWebsite {
                name: "youtube".into(),
                url: "https://www.youtube.com".into()
            })
        );
        assert_eq!(
            ToolCall::parse("get_file_info", &json!({"path": "C:\\doc.txt"})),
            Ok(ToolCall::GetFileInfo("C:\\doc.txt".into()))
        );
    }

    #[test]
    fn discovery_accepts_local_paths_and_mutation_paths_keep_internal_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().display().to_string();
        assert!(matches!(
            ToolCall::parse(
                "find_large_files",
                &json!({"folders":[path], "count":u64::MAX})
            )
            .unwrap(),
            ToolCall::FindLargeFiles {
                scope: Scope::Custom(_),
                count: 100
            }
        ));
        assert!(matches!(
            ToolCall::parse("find_large_files", &json!({})).unwrap(),
            ToolCall::FindLargeFiles {
                scope: Scope::AllDrives,
                ..
            }
        ));
        let source = format!("D:\\Work Projects\\{}\\report  final.txt", "a".repeat(140));
        assert_eq!(
            ToolCall::parse(
                "move_file",
                &json!({"source":source,"destination":"D:\\Archive"})
            )
            .unwrap(),
            ToolCall::MoveFile {
                source,
                destination: "D:\\Archive".into()
            }
        );
    }

    #[test]
    fn refuses_anything_outside_the_vocabulary() {
        for (name, args) in [
            ("run_command", json!({"command": "del /s"})),
            ("open_app", json!({"app": "cmd"})),
            ("open_app", json!({"app": "powershell.exe -c whoami"})),
            ("open_folder", json!({"folder": "C:\\Windows"})),
            (
                "find_large_files",
                json!({"folders": ["\\\\server\\share"]}),
            ),
            ("find_large_files", json!({"folders": "Downloads"})),
            ("find_large_files", json!({"folders": [7]})),
            ("open_website", json!({"site": "evil.com\"&calc"})),
            ("open_website", json!({"site": "javascript:alert(1)"})),
            ("open_website", json!({"site": "file:///C:/Windows"})),
            ("web_search", json!({})),
            ("web_search", json!({"query": "   "})),
            ("web_search", json!({"query": "x".repeat(101)})),
            ("find_files", json!({"name": 42})),
            ("check_disk_space", json!("not an object")),
        ] {
            assert!(
                ToolCall::parse(name, &args).is_err(),
                "{name} {args} should be refused"
            );
        }
    }

    /// Tools the agent answers itself, with no access to the PC: these never
    /// reach the desktop, so the desktop has no name for them. Anything not
    /// listed here must be one this desktop accepts.
    const AGENT_ONLY_TOOLS: &[&str] = &["calculate", "current_date_time"];

    /// The agent tells the model which tools exist; this desktop decides what
    /// they mean. If one is added there and not here the model can ask for
    /// something Cubby refuses, so the two lists are checked against each
    /// other. Argument errors are fine (this call passes none), an unknown
    /// tool is not.
    #[test]
    fn the_desktop_knows_every_tool_the_agent_offers() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../services/agent/src/tools.ts");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
        let mut checked = 0;
        let mut in_tools = false;
        for line in source.lines() {
            if line.starts_with("export const TOOLS") {
                in_tools = true;
                continue;
            }
            if in_tools && line.starts_with('}') {
                break;
            }
            // Only top-level entries: a tool key is indented by exactly two
            // spaces, while anything nested inside it is indented further
            // (so `params: {` is not mistaken for a tool called "params").
            let Some((name, _)) = line.split_once(": {") else {
                continue;
            };
            if !in_tools || !line.starts_with("  ") || line.starts_with("   ") {
                continue;
            }
            let name = name.trim();
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            if AGENT_ONLY_TOOLS.contains(&name) {
                continue;
            }
            let problem = ToolCall::parse(name, &json!({})).err().unwrap_or_default();
            assert!(
                !problem.contains("there is no tool called"),
                "the agent offers \"{name}\", which the desktop refuses; either teach \
                 ToolCall::parse about it, or add it to AGENT_ONLY_TOOLS if the agent \
                 answers it alone"
            );
            checked += 1;
        }
        assert!(checked >= 20, "only {checked} tools found in tools.ts");
        // The settings tools this feature added are among them.
        for name in ["list_settings", "change_setting"] {
            assert!(source.contains(name), "{name} is missing from the agent");
        }
    }

    /// The agent's description of `change_setting` is the only place the model
    /// learns which settings exist, and the model has to name one straight
    /// away (asking first cost it every settings case in the evaluation). A
    /// setting added to the catalogue but not to that description is invisible
    /// to the model, so the two are checked against each other.
    #[test]
    fn the_agent_knows_every_setting_cubby_can_change() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../services/agent/src/tools.ts");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
        for setting in custodian_winsec::settings::all() {
            assert!(
                source.contains(setting.id),
                "the agent does not tell the model about the setting \"{}\"; add it to the \
                 change_setting description in tools.ts",
                setting.id
            );
        }
    }

    #[test]
    fn a_setting_change_must_name_a_real_setting_and_one_of_its_values() {
        let known = custodian_winsec::settings::find("taskbar_alignment").unwrap();
        assert_eq!(
            ToolCall::parse(
                "change_setting",
                &json!({"setting": known.id, "value": "left"})
            ),
            Ok(ToolCall::ChangeSetting {
                setting: "taskbar_alignment".into(),
                value: "left".into()
            })
        );
        assert_eq!(
            ToolCall::parse("list_settings", &json!({})),
            Ok(ToolCall::ListSettings)
        );
        // Capitalisation is the person's business, not a reason to refuse.
        assert_eq!(
            ToolCall::parse(
                "change_setting",
                &json!({"setting": "dark_mode_apps", "value": "Light"})
            ),
            Ok(ToolCall::ChangeSetting {
                setting: "dark_mode_apps".into(),
                value: "Light".into()
            })
        );
        for (setting, value) in [
            ("no_such_setting", "on"),
            ("taskbar_alignment", "sideways"),
            ("C:\\Windows", "on"),
            ("dark_mode_apps", "lighthouse"),
        ] {
            assert!(
                ToolCall::parse(
                    "change_setting",
                    &json!({"setting": setting, "value": value})
                )
                .is_err(),
                "{setting} {value} should be refused"
            );
        }
        assert!(ToolCall::parse("change_setting", &json!({"setting": "dark_mode_apps"})).is_err());
        // The model can never put a setting back on its own.
        assert!(ToolCall::parse("revert_setting_change", &json!({})).is_err());
        assert!(!ToolCall::RevertSettingChange.model_may_request());
        assert_eq!(
            ToolCall::from_intent(&intent::parse("put back the setting")),
            Some(ToolCall::RevertSettingChange)
        );
        assert_eq!(
            ToolCall::from_intent(&intent::parse("list settings")),
            Some(ToolCall::ListSettings)
        );
    }

    #[test]
    fn the_model_can_never_empty_the_safe_place() {
        for name in [
            "empty_safe_place",
            "purge",
            "purge_now",
            "empty_the_safe_place",
        ] {
            assert!(ToolCall::parse(name, &json!({})).is_err(), "{name}");
        }
        assert!(!ToolCall::EmptySafePlace.model_may_request());
        assert!(ToolCall::UndoLastCleanup.model_may_request());
        assert_eq!(
            ToolCall::from_intent(&intent::parse("empty the safe place")),
            Some(ToolCall::EmptySafePlace)
        );
    }

    #[test]
    fn strips_control_and_invisible_characters_from_text() {
        assert_eq!(
            ToolCall::parse(
                "web_search",
                &json!({"query": "cheap\u{0}\nflights\u{1b}[2J"})
            ),
            Ok(ToolCall::WebSearch("cheap flights[2J".into()))
        );
        assert_eq!(
            ToolCall::parse("web_search", &json!({"query": "a\tb"})),
            Ok(ToolCall::WebSearch("a b".into()))
        );
        // A right-to-left override would make the card read "exe.pdf".
        assert_eq!(
            ToolCall::parse("find_files", &json!({"name": "invoice\u{202e}fdp.exe"})),
            Ok(ToolCall::FindFiles {
                name: "invoicefdp.exe".into(),
                scope: Scope::AllDrives
            })
        );
        assert!(ToolCall::parse("web_search", &json!({"query": "\u{200b}\u{feff}"})).is_err());
    }

    #[test]
    fn maps_rule_intents_to_the_same_tools() {
        assert_eq!(
            ToolCall::from_intent(&intent::parse("how much free space?")),
            Some(ToolCall::CheckDiskSpace)
        );
        assert_eq!(
            ToolCall::from_intent(&intent::parse("find the largest file on my system")),
            Some(ToolCall::FindLargeFiles {
                scope: Scope::AllDrives,
                count: 1,
            })
        );
        assert_eq!(
            ToolCall::from_intent(&intent::parse("biggest files in Downloads")),
            Some(ToolCall::FindLargeFiles {
                scope: Scope::Folders(vec![KnownFolder::Downloads]),
                count: 10,
            })
        );
        assert_eq!(
            ToolCall::from_intent(&intent::parse("where is my resume?")),
            Some(ToolCall::FindFiles {
                name: "resume".into(),
                scope: Scope::AllDrives
            })
        );
        assert_eq!(
            ToolCall::from_intent(&intent::parse("is my pc slow?")),
            Some(ToolCall::SystemStatus)
        );
        assert_eq!(
            ToolCall::from_intent(&intent::parse("file info for C:\\doc.txt")),
            Some(ToolCall::GetFileInfo("C:\\doc.txt".into()))
        );
        assert_eq!(ToolCall::from_intent(&intent::parse("hello")), None);
        assert_eq!(
            ToolCall::from_intent(&intent::parse("write a poem about rain")),
            None
        );
    }
}
