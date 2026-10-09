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
    FindLargeFiles(Scope),
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
    /// Frees the safe place's space now. Only the person can ask for this:
    /// the model has no name for it, and [`ToolCall::model_may_request`] says no.
    EmptySafePlace,
}

impl ToolCall {
    /// The tool for a request the rules understood, if it is one.
    pub fn from_intent(intent: &Intent) -> Option<Self> {
        Some(match intent {
            Intent::DiskSpace => ToolCall::CheckDiskSpace,
            Intent::LargestFiles(scope) => ToolCall::FindLargeFiles(scope.clone()),
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
            _ => return None,
        })
    }

    /// Whether the language model may ask for this at all. Removing the way
    /// back (emptying the safe place) is the person's decision alone.
    pub fn model_may_request(&self) -> bool {
        !matches!(self, ToolCall::EmptySafePlace | ToolCall::RemoveTask(_))
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
            "find_large_files" => ToolCall::FindLargeFiles(folders_arg(args)?),
            "find_duplicate_files" => ToolCall::FindDuplicateFiles(folders_arg(args)?),
            "find_files" => ToolCall::FindFiles {
                name: text_arg(args, "name")?,
                scope: folders_arg(args)?,
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
            Ok(ToolCall::FindLargeFiles(Scope::Folders(vec![
                KnownFolder::Downloads,
                KnownFolder::Videos
            ])))
        );
        assert_eq!(
            ToolCall::parse("find_duplicate_files", &json!({"folders": []})),
            Ok(ToolCall::FindDuplicateFiles(Scope::UserFolders))
        );
        assert_eq!(
            ToolCall::parse("find_files", &json!({"name": "  tax   2025 "})),
            Ok(ToolCall::FindFiles {
                name: "tax 2025".into(),
                scope: Scope::UserFolders
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
    }

    #[test]
    fn refuses_anything_outside_the_vocabulary() {
        for (name, args) in [
            ("run_command", json!({"command": "del /s"})),
            ("open_app", json!({"app": "cmd"})),
            ("open_app", json!({"app": "powershell.exe -c whoami"})),
            ("open_folder", json!({"folder": "C:\\Windows"})),
            ("find_large_files", json!({"folders": ["C:\\"]})),
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
                scope: Scope::UserFolders
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
            ToolCall::from_intent(&intent::parse("where is my resume?")),
            Some(ToolCall::FindFiles {
                name: "resume".into(),
                scope: Scope::UserFolders
            })
        );
        assert_eq!(
            ToolCall::from_intent(&intent::parse("is my pc slow?")),
            Some(ToolCall::SystemStatus)
        );
        assert_eq!(ToolCall::from_intent(&intent::parse("hello")), None);
        assert_eq!(
            ToolCall::from_intent(&intent::parse("write a poem about rain")),
            None
        );
    }
}
