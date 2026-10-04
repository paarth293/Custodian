//! Cubby's assistant: understands a chat message, runs the matching tool and
//! streams the answer back to the chat panel. Requests the rules don't
//! recognise go to the local language model through the agent process.

pub mod intent;
mod reply;

use crate::agent::{AgentError, AgentProcess};
use custodian_core::cleanup::{CleanupPlan, PlanItem, Quarantine};
use custodian_core::duplicates::{find_duplicates, Progress};
use custodian_core::largest::largest_files;
use custodian_core::walk::WalkLimits;
use intent::{App, Intent, KnownFolder, Scope};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

/// Maximum items in one clean-up plan (keeps the approval card reviewable).
const MAX_PLAN_ITEMS: usize = 1000;
/// Minimum size for duplicate detection; tiny files are rarely worth it.
const DUPLICATE_MIN_BYTES: u64 = 1024;

/// What the chat panel receives while Cubby answers.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ChatEvent {
    /// Text to append to Cubby's reply.
    Text { text: String },
    /// Progress shown while working ("Scanning… 12,000 files").
    Status { text: String },
    /// A clean-up waiting for approval.
    Plan { plan: PlanView },
    /// Something for Cubby itself to do (e.g. "sleep").
    Action { action: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationView {
    pub path: String,
    pub keep: String,
    pub action: &'static str,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub id: String,
    pub risk_level: &'static str,
    pub summary: String,
    pub total_bytes: u64,
    pub operations: Vec<OperationView>,
}

#[derive(Default)]
pub struct AssistantState {
    plans: Mutex<HashMap<String, CleanupPlan>>,
    agent: Mutex<Option<AgentProcess>>,
}

/// Where the assistant's events go: the chat panel's channel in the app, a
/// collecting closure in tests.
type Sink<'a> = &'a dyn Fn(ChatEvent);

fn emit(channel: Sink<'_>, event: ChatEvent) {
    channel(event);
}

fn say(channel: Sink<'_>, text: impl Into<String>) {
    emit(channel, ChatEvent::Text { text: text.into() });
}

fn folder_path(app: &AppHandle, folder: KnownFolder) -> Option<PathBuf> {
    let path = app.path();
    match folder {
        KnownFolder::Downloads => path.download_dir(),
        KnownFolder::Documents => path.document_dir(),
        KnownFolder::Desktop => path.desktop_dir(),
        KnownFolder::Pictures => path.picture_dir(),
        KnownFolder::Videos => path.video_dir(),
        KnownFolder::Music => path.audio_dir(),
    }
    .ok()
    .filter(|p| p.is_dir())
}

/// Existing folders for a scope, with their labels.
fn resolve_scope(app: &AppHandle, scope: &Scope) -> Vec<(KnownFolder, PathBuf)> {
    let folders: Vec<KnownFolder> = match scope {
        Scope::UserFolders => KnownFolder::ALL.to_vec(),
        Scope::Folders(list) => list.clone(),
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut resolved = Vec::new();
    for folder in folders {
        if let Some(path) = folder_path(app, folder) {
            // Skip folders nested in one already listed (e.g. Desktop inside OneDrive Documents).
            if seen
                .iter()
                .any(|s| path.starts_with(s) || s.starts_with(&path))
            {
                continue;
            }
            seen.push(path.clone());
            resolved.push((folder, path));
        }
    }
    resolved
}

fn quarantine(app: &AppHandle) -> Result<Quarantine, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    Ok(Quarantine::new(dir.join("quarantine")))
}

/// Rate-limits status updates so a fast scan doesn't flood the UI.
struct Throttle(Instant);

impl Throttle {
    fn ready(&mut self) -> bool {
        if self.0.elapsed() >= Duration::from_millis(250) {
            self.0 = Instant::now();
            true
        } else {
            false
        }
    }
}

fn find_duplicates_tool(app: &AppHandle, state: &AssistantState, scope: &Scope, channel: Sink<'_>) {
    let folders = resolve_scope(app, scope);
    if let Some(plan) = run_duplicates(&folders, channel) {
        state
            .plans
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(plan.id.clone(), plan);
    }
}

/// Scans `folders` for duplicates, reports them, and proposes a clean-up plan
/// (returned so the caller can keep it for approval).
fn run_duplicates(folders: &[(KnownFolder, PathBuf)], channel: Sink<'_>) -> Option<CleanupPlan> {
    if folders.is_empty() {
        say(channel, "I couldn't find those folders on this PC.");
        return None;
    }
    let labels: Vec<&str> = folders.iter().map(|(f, _)| f.label()).collect();
    emit(
        channel,
        ChatEvent::Status {
            text: format!("Looking for duplicates in {}…", reply::join_list(&labels)),
        },
    );

    let roots: Vec<PathBuf> = folders.iter().map(|(_, p)| p.clone()).collect();
    let mut throttle = Throttle(Instant::now());
    let report = find_duplicates(
        &roots,
        WalkLimits {
            min_size: DUPLICATE_MIN_BYTES,
            ..WalkLimits::default()
        },
        &AtomicBool::new(false),
        &mut |p| {
            if throttle.ready() {
                let text = match p {
                    Progress::Scanning { files } => {
                        format!("Scanning… {} files", reply::count(files))
                    }
                    Progress::Comparing { done, total } => {
                        format!("Comparing possible matches… {done} of {total}")
                    }
                };
                emit(channel, ChatEvent::Status { text });
            }
        },
    );

    say(channel, reply::duplicates_summary(&report, &labels));
    if report.groups.is_empty() {
        return None;
    }

    let items: Vec<PlanItem> = report
        .groups
        .iter()
        .flat_map(|g| {
            let keep = g.files[0].path.clone();
            g.files[1..].iter().map(move |f| PlanItem {
                path: f.path.clone(),
                keep: keep.clone(),
                size: g.size,
                hash: g.hash.clone(),
            })
        })
        .take(MAX_PLAN_ITEMS)
        .collect();
    let plan = CleanupPlan::new(items);
    let view = PlanView {
        id: plan.id.clone(),
        risk_level: "low",
        summary: format!(
            "Move {} duplicate {} ({}) to Cubby's safe place. The original of each stays where it is, and you can undo this anytime.",
            reply::count(plan.items.len()),
            reply::plural(plan.items.len(), "copy", "copies"),
            reply::bytes(plan.total_bytes())
        ),
        total_bytes: plan.total_bytes(),
        operations: plan
            .items
            .iter()
            .map(|i| OperationView {
                path: i.path.display().to_string(),
                keep: i.keep.display().to_string(),
                action: "quarantine",
                size_bytes: i.size,
            })
            .collect(),
    };
    emit(channel, ChatEvent::Plan { plan: view });
    Some(plan)
}

fn largest_files_tool(app: &AppHandle, scope: &Scope, channel: Sink<'_>) {
    run_largest(&resolve_scope(app, scope), channel);
}

fn run_largest(folders: &[(KnownFolder, PathBuf)], channel: Sink<'_>) {
    if folders.is_empty() {
        say(channel, "I couldn't find those folders on this PC.");
        return;
    }
    let labels: Vec<&str> = folders.iter().map(|(f, _)| f.label()).collect();
    emit(
        channel,
        ChatEvent::Status {
            text: format!("Measuring files in {}…", reply::join_list(&labels)),
        },
    );
    let roots: Vec<PathBuf> = folders.iter().map(|(_, p)| p.clone()).collect();
    let mut throttle = Throttle(Instant::now());
    let (files, stats) = largest_files(
        &roots,
        10,
        WalkLimits::default(),
        &AtomicBool::new(false),
        &mut |n| {
            if throttle.ready() {
                emit(
                    channel,
                    ChatEvent::Status {
                        text: format!("Scanning… {} files", reply::count(n)),
                    },
                );
            }
        },
    );
    say(
        channel,
        reply::largest_summary(&files, &labels, stats.files_seen),
    );
}

fn disk_space_tool(channel: Sink<'_>) {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut drives: Vec<(String, u64, u64)> = disks
        .list()
        .iter()
        .filter(|d| d.total_space() > 0 && !d.is_removable())
        .map(|d| {
            (
                d.mount_point().display().to_string(),
                d.available_space(),
                d.total_space(),
            )
        })
        .collect();
    drives.sort();
    drives.dedup_by(|a, b| a.0 == b.0);
    say(channel, reply::disk_summary(&drives));
}

fn open_app(app_kind: App) -> Result<(), String> {
    #[cfg(windows)]
    {
        let exe = match app_kind {
            App::Notepad => "notepad.exe",
            App::Calculator => "calc.exe",
            App::Paint => "mspaint.exe",
            App::FileExplorer => "explorer.exe",
            App::TaskManager => "taskmgr.exe",
            App::Settings => return Err("settings".into()),
        };
        std::process::Command::new(exe)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = app_kind;
        Err("opening apps is supported on Windows".into())
    }
}

/// Percent-encodes a search query for a URL.
fn encode_query(query: &str) -> String {
    query
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn agent_entry(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CUSTODIAN_AGENT_ENTRY") {
        return Some(PathBuf::from(path));
    }
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("agent").join("server.js"));
    let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../services/agent/dist/server.js");
    [bundled, Some(dev)]
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
}

fn models_dir(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("CUSTODIAN_MODELS_DIR") {
        return Some(PathBuf::from(dir));
    }
    let bundled = app.path().resource_dir().ok().map(|d| d.join("models"));
    let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../models");
    [bundled, Some(dev)]
        .into_iter()
        .flatten()
        .find(|p| p.is_dir())
}

/// Answers open-ended messages with the local model.
fn ask_model(app: &AppHandle, state: &AssistantState, message: &str, channel: Sink<'_>) {
    let mut agent = state.agent.lock().unwrap_or_else(|e| e.into_inner());
    if agent.as_mut().is_some_and(|a| !a.is_alive()) {
        *agent = None;
    }
    if agent.is_none() {
        let Some(entry) = agent_entry(app) else {
            say(
                channel,
                reply::no_brain(
                    "the agent isn't built yet (run `npm run build` in the repository)",
                ),
            );
            return;
        };
        emit(
            channel,
            ChatEvent::Status {
                text: "Waking up my brain… the first answer takes a moment.".into(),
            },
        );
        let node = std::env::var("CUSTODIAN_NODE").unwrap_or_else(|_| "node".into());
        match AgentProcess::spawn(&node, &entry, models_dir(app).as_ref()) {
            Ok(process) => *agent = Some(process),
            Err(err) => {
                say(channel, reply::no_brain(&err.to_string()));
                return;
            }
        }
    }
    let Some(process) = agent.as_mut() else {
        return;
    };
    emit(
        channel,
        ChatEvent::Status {
            text: "Thinking…".into(),
        },
    );
    let result = process.chat(message, &mut |text| say(channel, text));
    match result {
        Ok(()) => {}
        Err(AgentError::Model(reason)) => say(channel, reply::no_brain(&reason)),
        Err(err) => {
            *agent = None;
            say(channel, reply::no_brain(&err.to_string()));
        }
    }
}

/// Handles one chat message from start to finish.
fn respond(app: &AppHandle, state: &AssistantState, message: &str, channel: Sink<'_>) {
    match intent::parse(message) {
        Intent::Greeting => say(channel, reply::GREETING),
        Intent::Thanks => say(channel, "You're welcome! Anything else I can do?"),
        Intent::Help => say(channel, reply::HELP),
        Intent::WhoAreYou => say(channel, reply::WHO_AM_I),
        Intent::Time => say(channel, format!("It's {}.", chrono::Local::now().format("%-I:%M %p"))),
        Intent::Date => say(channel, format!("Today is {}.", chrono::Local::now().format("%A, %-d %B %Y"))),
        Intent::DiskSpace => disk_space_tool(channel),
        Intent::FindDuplicates(scope) => find_duplicates_tool(app, state, &scope, channel),
        Intent::LargestFiles(scope) => largest_files_tool(app, &scope, channel),
        Intent::OpenWebsite { name, url } => match app.opener().open_url(&url, None::<&str>) {
            Ok(()) => say(channel, format!("Opening {} in your browser.", reply::title_case(&name))),
            Err(err) => say(channel, format!("I couldn't open {url}: {err}")),
        },
        Intent::WebSearch(query) => {
            let url = format!("https://www.google.com/search?q={}", encode_query(&query));
            match app.opener().open_url(&url, None::<&str>) {
                Ok(()) => say(channel, format!("Searching the web for \"{query}\" in your browser.")),
                Err(err) => say(channel, format!("I couldn't open your browser: {err}")),
            }
        }
        Intent::OpenApp(kind) => {
            let result = match open_app(kind) {
                Err(e) if e == "settings" => app
                    .opener()
                    .open_url("ms-settings:", None::<&str>)
                    .map_err(|e| e.to_string()),
                other => other,
            };
            match result {
                Ok(()) => say(channel, format!("Opening {}.", kind.label())),
                Err(err) => say(channel, format!("I couldn't open {}: {err}", kind.label())),
            }
        }
        Intent::OpenFolder(folder) => match folder_path(app, folder) {
            Some(path) => match app.opener().open_path(path.display().to_string(), None::<&str>) {
                Ok(()) => say(channel, format!("Opening your {} folder.", folder.label())),
                Err(err) => say(channel, format!("I couldn't open {}: {err}", folder.label())),
            },
            None => say(channel, format!("I couldn't find a {} folder on this PC.", folder.label())),
        },
        Intent::OpenUnknown(target) => say(
            channel,
            format!(
                "I don't know an app or website called \"{target}\" yet. Try \"open {target}.com\" or \"search {target}\"."
            ),
        ),
        Intent::Undo => match quarantine(app).and_then(|q| q.undo_last().map_err(|e| e.to_string())) {
            Ok(outcome) => say(channel, reply::undo_summary(&outcome)),
            Err(err) => say(channel, reply::undo_error(&err)),
        },
        Intent::History => match quarantine(app) {
            Ok(q) => say(channel, reply::history_summary(&q.history())),
            Err(err) => say(channel, format!("I couldn't read my history: {err}")),
        },
        Intent::Sleep => {
            say(channel, "Okay, taking a little nap. Move your mouse near me to wake me up.");
            emit(channel, ChatEvent::Action { action: "sleep".into() });
        }
        Intent::Unknown => ask_model(app, state, message, channel),
    }
}

#[tauri::command]
pub async fn send_chat_message(
    app: AppHandle,
    message: String,
    channel: Channel<ChatEvent>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        // A closed chat panel is not an error.
        let sink = |event: ChatEvent| {
            let _ = channel.send(event);
        };
        respond(&app, &state, &message, &sink);
    })
    .await
    .map_err(|e| e.to_string())
}

/// Executes a clean-up plan that Cubby proposed. Only plans created by the
/// assistant itself can be approved; the webview never supplies paths.
#[tauri::command]
pub async fn approve_plan(app: AppHandle, plan_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        let plan = state
            .plans
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&plan_id)
            .ok_or("That plan is no longer available. Ask me to scan again.")?;
        let outcome = quarantine(&app)?
            .execute(&plan)
            .map_err(|e| e.to_string())?;
        Ok(reply::cleanup_summary(&outcome))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn reject_plan(state: tauri::State<'_, AssistantState>, plan_id: String) {
    state
        .plans
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&plan_id);
}

/// Past clean-ups for the history view.
#[tauri::command]
pub fn get_ledger(app: AppHandle) -> Result<Vec<String>, String> {
    Ok(quarantine(&app)?
        .history()
        .iter()
        .map(reply::history_line)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;

    #[test]
    fn duplicates_flow_end_to_end_scan_approve_undo() {
        let dir = tempfile::tempdir().unwrap();
        let downloads = dir.path().join("Downloads");
        let pictures = dir.path().join("Pictures");
        fs::create_dir_all(downloads.join("old")).unwrap();
        fs::create_dir_all(&pictures).unwrap();
        let photo = vec![42u8; 4096];
        fs::write(pictures.join("holiday.jpg"), &photo).unwrap();
        fs::write(downloads.join("holiday (1).jpg"), &photo).unwrap();
        fs::write(downloads.join("old/holiday copy.jpg"), &photo).unwrap();
        fs::write(downloads.join("unique.pdf"), vec![1u8; 4096]).unwrap();

        // 1. "find duplicates": Cubby reports and proposes a plan.
        let events = RefCell::new(Vec::new());
        let sink = |e: ChatEvent| events.borrow_mut().push(e);
        let folders = vec![
            (KnownFolder::Pictures, pictures.clone()),
            (KnownFolder::Downloads, downloads.clone()),
        ];
        let plan = run_duplicates(&folders, &sink).expect("a plan for the duplicates");
        let events = events.into_inner();
        let text: String = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(text.contains("found 2 extra copies of 1 file"), "{text}");
        assert!(events.iter().any(|e| matches!(e, ChatEvent::Status { .. })));
        let view = events
            .iter()
            .find_map(|e| match e {
                ChatEvent::Plan { plan } => Some(plan),
                _ => None,
            })
            .expect("plan event");
        assert_eq!(view.operations.len(), 2);
        assert!(view.operations.iter().all(|op| op.action == "quarantine"));

        // 2. Approve: the extra copies move to the safe place, the original stays.
        let quarantine = Quarantine::new(dir.path().join("quarantine"));
        let outcome = quarantine.execute(&plan).unwrap();
        assert_eq!(outcome.moved, 2);
        let kept = PathBuf::from(&view.operations[0].keep);
        assert!(kept.exists());
        assert!(view
            .operations
            .iter()
            .all(|op| op.keep == view.operations[0].keep));
        assert!(view
            .operations
            .iter()
            .all(|op| !PathBuf::from(&op.path).exists()));
        assert!(downloads.join("unique.pdf").exists());
        assert!(reply::cleanup_summary(&outcome).contains("moved 2 duplicate copies"));

        // 3. "undo": everything is back, byte for byte.
        let undo = quarantine.undo_last().unwrap();
        assert_eq!(undo.restored, 2);
        for op in &view.operations {
            assert_eq!(fs::read(&op.path).unwrap(), photo);
        }
        assert_eq!(fs::read(&kept).unwrap(), photo);
    }

    #[test]
    fn largest_files_lists_biggest_first() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("movie.mp4"), vec![0u8; 50_000]).unwrap();
        fs::write(dir.path().join("note.txt"), vec![0u8; 10]).unwrap();
        let events = RefCell::new(Vec::new());
        let sink = |e: ChatEvent| events.borrow_mut().push(e);
        run_largest(&[(KnownFolder::Videos, dir.path().to_path_buf())], &sink);
        let text = match events.into_inner().pop() {
            Some(ChatEvent::Text { text }) => text,
            other => panic!("expected text, got {other:?}"),
        };
        let movie = text.find("movie.mp4").unwrap();
        let note = text.find("note.txt").unwrap();
        assert!(movie < note);
    }

    #[test]
    fn encodes_search_queries() {
        assert_eq!(encode_query("cheap flights"), "cheap+flights");
        assert_eq!(encode_query("a&b=c"), "a%26b%3Dc");
    }
}
