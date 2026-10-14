#![allow(unused_imports, dead_code)]
//! Cubby's assistant: understands a chat message, runs the matching tool and
//! streams the answer back to the chat panel. Requests the rules don't
//! recognise go to the local language model through the agent process, which
//! may ask for the same tools (checked again in [`tools`]).

mod actions;
mod advice;
mod apps;
pub mod intent;
mod launcher;
mod reply;
mod settings;
mod system;
mod tasks;
mod tools;
mod when;

use crate::agent::{AgentError, AgentProcess};
use crate::approval_service::ApprovalService;
use crate::history_ledger;
use actions::{ActionCard, PendingActions};
use custodian_core::cleanup::{duplicate_plan_items, CleanupOutcome, CleanupPlan, Quarantine};
use custodian_core::duplicates::{find_duplicates, Progress};
use custodian_core::find::find_by_name;
use custodian_core::largest::largest_files;
use custodian_core::temp::{self, TempOutcome, TempPlan};
use custodian_core::walk::WalkLimits;
use intent::{App, Intent, KnownFolder, Scope};
use serde::Serialize;
use serde_json::json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use tools::ToolCall;

/// Maximum items in one clean-up plan (keeps the approval card reviewable).
const MAX_PLAN_ITEMS: usize = 1000;
/// Minimum size for duplicate detection; tiny files are rarely worth it.
const DUPLICATE_MIN_BYTES: u64 = 1024;
/// Most files listed for "where's my …?".
const MAX_FOUND_FILES: usize = 10;

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
    /// What a tool found when the language model asked for it, written by
    /// Cubby's own code (never by the model) and shown as its own block.
    Result { text: String },
    /// Something the model asked for that waits for the person's click.
    Confirm { card: ActionCard },
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
    /// Every file in the plan; `operations` may list only the first of them.
    pub file_count: usize,
    pub operations: Vec<OperationView>,
}

/// Most files a plan card lists; the rest are counted.
const MAX_VIEW_OPERATIONS: usize = 200;

/// A plan waiting for the person's approval.
#[derive(Debug, Clone)]
enum PendingPlan {
    /// Move duplicate copies to the safe place.
    Duplicates(CleanupPlan),
    /// Delete old temporary files.
    Temp(TempPlan),
}

impl PendingPlan {
    fn digest(&self) -> custodian_approval::PlanHash {
        match self {
            PendingPlan::Duplicates(plan) => plan.digest(),
            PendingPlan::Temp(plan) => plan.digest(),
        }
    }

    /// The question shown in the approval prompt.
    fn question(&self) -> String {
        match self {
            PendingPlan::Duplicates(plan) => approval_question(plan),
            PendingPlan::Temp(plan) => reply::temp_question(plan.items.len(), plan.total_bytes()),
        }
    }
}

/// What an approved plan did.
#[derive(Debug, Clone)]
enum PlanOutcome {
    Cleanup(CleanupOutcome),
    Temp(TempOutcome),
}

#[derive(Default)]
pub struct AssistantState {
    plans: Mutex<HashMap<String, PendingPlan>>,
    agent: Mutex<Option<AgentProcess>>,
    /// When the agent was last started or asked something.
    agent_used: Mutex<Option<Instant>>,
    approvals: Mutex<Option<Arc<ApprovalService>>>,
    /// Model requests waiting for the person's click.
    actions: PendingActions,
    /// Set by the Stop button; scans and the model check it and stop.
    cancel: AtomicBool,
    /// One reader or writer of the to-do list at a time.
    tasks_lock: Mutex<()>,
}

/// Who asked for a tool: the person (by typing a command or clicking a
/// card), the language model (which may only ask), or the person's click on a
/// card (which is the answer to a request that already asked).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Person,
    Model,
    Approved,
}

/// Whether a call must wait for the person's click (see [`actions::card_text`]).
/// The model always asks before anything that sends data off the PC or changes
/// something; a Windows setting is shown even when the person asked for it
/// themselves, because getting it wrong changes their own desktop. A call they
/// have already approved on a card is never asked about twice.
fn needs_card(call: &ToolCall, origin: Origin) -> bool {
    match origin {
        Origin::Approved => false,
        Origin::Model => actions::card_text(call).is_some(),
        Origin::Person => matches!(call, ToolCall::ChangeSetting { .. }),
    }
}

/// Shown when the person pressed Stop during a search.
const STOPPED: &str = "Okay, I stopped.";

/// Shown when a card was already used, or waited too long.
const CARD_GONE: &str = "That request has expired. Ask me again if you still want it.";

/// The language model is unloaded after this long without a question (D-26).
const AGENT_IDLE_LIMIT: Duration = Duration::from_secs(10 * 60);

/// How often the idle check runs.
const AGENT_IDLE_CHECK: Duration = Duration::from_secs(60);

/// True when a loaded agent has gone unused for `limit`.
fn agent_is_idle(loaded: bool, last_used: Option<Instant>, now: Instant, limit: Duration) -> bool {
    loaded && last_used.is_none_or(|t| now.saturating_duration_since(t) >= limit)
}

/// Unloads the language model after [`AGENT_IDLE_LIMIT`] without use, so its
/// memory goes back to the system. It starts again on the next question.
pub fn start_idle_unloader(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(AGENT_IDLE_CHECK);
        let state = app.state::<AssistantState>();
        let mut agent = state.agent.lock().unwrap_or_else(|e| e.into_inner());
        let last_used = *state.agent_used.lock().unwrap_or_else(|e| e.into_inner());
        if agent_is_idle(agent.is_some(), last_used, Instant::now(), AGENT_IDLE_LIMIT) {
            *agent = None; // Dropping it ends the process and its job.
        }
    });
}

fn mark_agent_used(state: &AssistantState) {
    *state.agent_used.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}

/// Shown when the plan was cancelled, already run, or never existed.
const PLAN_GONE: &str = "That plan is no longer available. Ask me to scan again.";

/// Shown when the person closes the native dialog without saying yes.
const NOT_CONFIRMED: &str =
    "You didn't confirm, so nothing changed. You can approve again, or choose \"Not now\".";

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

/// Adds `event` to the history ledger; returns a note for the reply if the
/// history needs the person's attention.
fn record_history(app: &AppHandle, event: serde_json::Value) -> Option<String> {
    let result = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())
        .and_then(|dir| history_ledger::record(&dir.join("ledger"), &event));
    history_ledger::note(&result)
}

/// Appends an optional note as a new paragraph.
fn with_note(text: String, note: Option<String>) -> String {
    match note {
        Some(note) => format!(
            "{text}

{note}"
        ),
        None => text,
    }
}

/// The approval service, loading the sealed key on first use.
fn approvals(app: &AppHandle, state: &AssistantState) -> Result<Arc<ApprovalService>, String> {
    let mut slot = state.approvals.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(service) = slot.as_ref() {
        return Ok(Arc::clone(service));
    }
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let service = Arc::new(
        ApprovalService::load(&dir.join("keys").join("approval.key.dpapi"))
            .map_err(|e| format!("I couldn't open my approval key: {e}"))?,
    );
    *slot = Some(Arc::clone(&service));
    Ok(service)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// What happens to a clean-up's files, said the same way on the plan card
/// and in the confirmation (no space is freed until the window ends, EV-01).
fn safe_place_terms() -> String {
    format!(
        "The original of each file stays where it is. The copies wait in the safe place for {} days, and you can undo this anytime until then; after that their space is freed.",
        custodian_core::cleanup::QUARANTINE_DAYS
    )
}

/// The question shown in the native approval dialog.
fn approval_question(plan: &CleanupPlan) -> String {
    let n = plan.items.len();
    format!(
        "Move {} duplicate {} ({}) to Cubby's safe place?\n\n{}",
        reply::count(n),
        reply::plural(n, "copy", "copies"),
        reply::bytes(plan.total_bytes()),
        safe_place_terms()
    )
}

/// Asks the person to approve: with Windows Hello when it is set up (a system
/// prompt no other program can answer), otherwise in a native dialog. If Hello
/// is set up but fails, the approval is refused rather than weakened.
fn confirm_with_person(app: &AppHandle, title: &str, question: &str) -> bool {
    use custodian_winsec::hello::{self, HelloOutcome};
    if !hello::available() {
        return custodian_winsec::dialog::confirm(title, question);
    }
    let owner = app
        .get_webview_window(crate::cubby_window::CUBBY_LABEL)
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize)
        .unwrap_or(0);
    hello::verify(owner, question) == HelloOutcome::Verified
}

/// Approves and runs a pending plan: the person confirms through `confirm`
/// (the native dialog in the app), the plan's digest is signed, and the
/// quarantine verifies the signature before moving anything.
fn approve_and_run(
    plans: &Mutex<HashMap<String, PendingPlan>>,
    plan_id: &str,
    approvals: &ApprovalService,
    quarantine: &Quarantine,
    confirm: impl FnOnce(&str) -> bool,
) -> Result<PlanOutcome, String> {
    let lock = || plans.lock().unwrap_or_else(|e| e.into_inner());
    // The plan stays pending while the dialog is open, so "Not now" in the
    // chat panel still cancels it.
    let question = lock()
        .get(plan_id)
        .map(PendingPlan::question)
        .ok_or(PLAN_GONE)?;
    if !confirm(&question) {
        return Err(NOT_CONFIRMED.into());
    }
    // Claimed only after "yes": a plan cancelled while the dialog was open,
    // or already run by another click, is gone by now.
    let plan = lock().remove(plan_id).ok_or(PLAN_GONE)?;
    // The approval's lifetime starts when the person answered.
    let approval = approvals.sign(&plan.digest(), now_unix());
    match plan {
        PendingPlan::Duplicates(plan) => quarantine
            .execute_approved(&plan, &approval, approvals.verifier(), now_unix())
            .map(PlanOutcome::Cleanup)
            .map_err(|e| e.to_string()),
        PendingPlan::Temp(plan) => {
            temp::delete_approved(&plan, &approval, approvals.verifier(), now_unix())
                .map(PlanOutcome::Temp)
                .map_err(|e| e.to_string())
        }
    }
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

/// Runs one tool. When the language model asked for something that sends
/// data off the PC or changes files, a card is shown instead and the model is
/// told it is waiting; the returned note is that message. Otherwise `None`:
/// what the tool showed is the answer.
fn execute(
    app: &AppHandle,
    state: &AssistantState,
    call: ToolCall,
    origin: Origin,
    channel: Sink<'_>,
) -> Option<String> {
    if origin == Origin::Model && !call.model_may_request() {
        return Some("Only the person can ask for that; tell them how instead.".into());
    }
    if needs_card(&call, origin) {
        if let Some(card) = state.actions.add(call.clone(), Instant::now()) {
            let note = actions::waiting_note(&call);
            emit(channel, ChatEvent::Confirm { card });
            return Some(note);
        }
    }
    match call {
        ToolCall::CheckDiskSpace => disk_space_tool(channel),
        ToolCall::FindLargeFiles(scope) => {
            run_largest(&resolve_scope(app, &scope), &state.cancel, channel)
        }
        ToolCall::FindDuplicateFiles(scope) => find_duplicates_tool(app, state, &scope, channel),
        ToolCall::FindFiles { name, scope } => {
            run_find(&resolve_scope(app, &scope), &name, &state.cancel, channel)
        }
        ToolCall::SystemStatus => {
            emit(
                channel,
                ChatEvent::Status {
                    text: "Checking how your PC is doing…".into(),
                },
            );
            say(channel, reply::system_summary(&system::snapshot()));
        }
        ToolCall::OpenApp(kind) => open_app_tool(app, kind, channel),
        ToolCall::OpenFolder(folder) => open_folder_tool(app, folder, channel),
        ToolCall::WebSearch(query) => web_search_tool(app, &query, channel),
        ToolCall::OpenWebsite { name, url } => open_website_tool(app, &name, &url, channel),
        ToolCall::CleanupHistory => match quarantine(app) {
            Ok(q) => say(channel, reply::history_summary(&q.history())),
            Err(err) => say(channel, format!("I couldn't read my history: {err}")),
        },
        ToolCall::UndoLastCleanup => undo_tool(app, channel),
        ToolCall::EmptySafePlace => empty_safe_place_tool(app, channel),
        ToolCall::ListOpenApps => say(
            channel,
            reply::open_apps_summary(&apps::group(&custodian_winsec::apps::open_windows())),
        ),
        ToolCall::CloseApp(name) => close_app_tool(&name, channel),
        ToolCall::AddTask { text, when } => {
            add_task_tool(app, state, &text, when.as_deref(), channel)
        }
        ToolCall::ListTasks => match update_tasks(app, state, |list| (list.clone(), false)) {
            Ok(list) => say(channel, tasks::list_text(&list, chrono::Local::now())),
            Err(err) => say(channel, err),
        },
        ToolCall::CompleteTask(name) => finish_task_tool(app, state, &name, false, channel),
        ToolCall::RemoveTask(name) => finish_task_tool(app, state, &name, true, channel),
        ToolCall::CleanTempFiles => clean_temp_tool(state, channel),
        ToolCall::Suggestions => suggestions_tool(app, state, channel),
        ToolCall::SecurityStatus => say(channel, reply::security_summary(&protections(app))),
        ToolCall::OpenProgram(name) => open_program_tool(app, state, &name, channel),
        ToolCall::OpenFile(name) => open_file_tool(app, state, &name, false, channel),
        ToolCall::ListSettings => say(channel, settings::list_text()),
        ToolCall::ChangeSetting { setting, value } => {
            change_setting_tool(app, &setting, &value, channel)
        }
        ToolCall::RevertSettingChange => revert_setting_tool(app, channel),
    }
    None
}

/// Where the sealed record of settings changes is kept.
fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|dir| dir.join("settings.dpapi"))
        .map_err(|e| e.to_string())
}

/// Writes a checked settings change and records what it replaced, so it can be
/// put back. The change is written first and recorded second; when the record
/// cannot be written the person is told plainly that Cubby cannot put it back
/// for them, rather than being left to find out later.
fn apply_setting_change(
    app: &AppHandle,
    change: &custodian_winsec::settings::Change,
    channel: Sink<'_>,
) {
    let Some(setting) = change.setting() else {
        return say(channel, settings::unknown_text(&change.id));
    };
    if let Err(err) = setting.apply(&change.value) {
        return say(channel, err.to_string());
    }
    let now = now_unix() as i64;
    let recorded = settings_path(app).and_then(|path| {
        let mut history = settings::load(&path).map_err(|e| e.to_string())?;
        history.record(change.clone(), now);
        settings::save(&path, &mut history, now).map_err(|e| e.to_string())
    });
    let note = record_history(
        app,
        json!({
            "kind": "setting_change",
            "setting": change.id,
            "value": change.value,
            "from": change.from,
            "at": now,
        }),
    );
    let mut text = settings::changed_text(change);
    if let Err(err) = recorded {
        text.push_str(&format!(
            "\n\nHeads up: I couldn't write down what it was before ({err}), so I can't put this back for you — you can set it back the same way you changed it."
        ));
    }
    say(channel, with_note(text, note));
}

/// Changes one setting, or refuses a setting or value that does not exist.
fn change_setting_tool(app: &AppHandle, id: &str, value: &str, channel: Sink<'_>) {
    let Some(setting) = custodian_winsec::settings::find(id) else {
        return say(channel, settings::unknown_text(id));
    };
    let change = match setting.plan(value) {
        Ok(change) => change,
        Err(err) => return say(channel, err.to_string()),
    };
    if change.is_noop() {
        return say(channel, settings::already_text(&change));
    }
    apply_setting_change(app, &change, channel);
}

/// Puts back the most recent settings change, from the sealed record.
fn revert_setting_tool(app: &AppHandle, channel: Sink<'_>) {
    let path = match settings_path(app) {
        Ok(path) => path,
        Err(err) => {
            return say(
                channel,
                format!("I couldn't open my record of settings changes: {err}"),
            )
        }
    };
    let mut history = match settings::load(&path) {
        Ok(history) => history,
        Err(err) => {
            return say(
                channel,
                format!("I couldn't read my record of settings changes: {err}"),
            )
        }
    };
    let Some(entry) = history.last_open().cloned() else {
        return say(channel, settings::NOTHING_TO_REVERT);
    };
    if let Err(err) = custodian_winsec::settings::revert(&entry.change) {
        return say(channel, format!("I couldn't put that back: {err}"));
    }
    history.mark_reverted(entry.id);
    let now = now_unix();
    let saved = settings::save(&path, &mut history, now_unix() as i64).err();
    let note = record_history(
        app,
        json!({"kind": "setting_revert", "setting": entry.change.id, "at": now}),
    );
    let mut text = settings::reverted_text(&entry.change);
    if let Some(err) = saved {
        text.push_str(&format!(
            "\n\nHeads up: I put it back, but couldn't update my own record ({err}), so I may offer to put this back again."
        ));
    }
    say(channel, with_note(text, note));
}

/// Handles a request about settings, or answers `false` when the words fit
/// nothing Cubby may change, so the language model can try instead.
fn resolve_settings_request(
    app: &AppHandle,
    state: &AssistantState,
    words: &str,
    channel: Sink<'_>,
) -> bool {
    match settings::resolve(words) {
        settings::Resolution::Unknown => false,
        settings::Resolution::Ready(setting, value) => {
            execute(
                app,
                state,
                ToolCall::ChangeSetting {
                    setting: setting.id.to_string(),
                    value,
                },
                Origin::Person,
                channel,
            );
            true
        }
        settings::Resolution::NeedsValue(setting) => {
            say(channel, settings::needs_value_text(setting));
            true
        }
        settings::Resolution::Ambiguous(found) => {
            say(channel, settings::which_text(&found));
            true
        }
    }
}

/// "Open Spotify": an app from the Start menu. If no app has that name, the
/// person may mean one of their files ("open my resume").
fn open_program_tool(app: &AppHandle, state: &AssistantState, name: &str, channel: Sink<'_>) {
    let apps = launcher::start_apps();
    match launcher::find(&apps, name).as_slice() {
        [one] => match launcher::launch(one) {
            Ok(()) => say(channel, format!("Opening {}.", reply::safe_name(&one.name))),
            Err(err) => say(
                channel,
                format!("I couldn't open {}: {err}", reply::safe_name(&one.name)),
            ),
        },
        [] => open_file_tool(app, state, name, true, channel),
        many => {
            let names: Vec<String> = many
                .iter()
                .take(5)
                .map(|a| reply::safe_name(&a.name))
                .collect();
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            say(
                channel,
                format!("Which one do you mean: {}?", reply::join_list(&names)),
            );
        }
    }
}

/// "Open my resume": one of the person's documents or media files, with its
/// usual app. Programs and scripts are never opened. With `fallback`, the
/// name was not an app either, and the reply says so.
fn open_file_tool(
    app: &AppHandle,
    state: &AssistantState,
    name: &str,
    fallback: bool,
    channel: Sink<'_>,
) {
    let folders = resolve_scope(app, &Scope::UserFolders);
    let roots: Vec<PathBuf> = folders.iter().map(|(_, p)| p.clone()).collect();
    emit(
        channel,
        ChatEvent::Status {
            text: format!("Looking for \"{}\"…", reply::safe_name(name)),
        },
    );
    let (found, _) = find_by_name(
        &roots,
        name,
        MAX_FOUND_FILES,
        WalkLimits::default(),
        &state.cancel,
        &mut |_| {},
    );
    if state.cancel.load(Ordering::Relaxed) {
        return say(channel, STOPPED);
    }
    let openable: Vec<_> = found
        .iter()
        .filter(|f| launcher::openable(&f.path))
        .collect();
    match openable.as_slice() {
        [one] => match app
            .opener()
            .open_path(one.path.display().to_string(), None::<&str>)
        {
            Ok(()) => say(
                channel,
                format!("Opening {}.", reply::file_name(&one.path)),
            ),
            Err(err) => say(
                channel,
                format!("I couldn't open {}: {err}", reply::file_name(&one.path)),
            ),
        },
        [] if !found.is_empty() => say(
            channel,
            "For safety I only open documents, photos, music and videos, not programs or scripts. You can open it yourself from its folder.",
        ),
        [] => {
            let target = reply::safe_name(name);
            say(
                channel,
                if fallback {
                    format!(
                        "I don't know an app, website or file called \"{target}\". Try \"open {target}.com\" or \"search {target}\"."
                    )
                } else {
                    format!("I couldn't find a file called \"{target}\" in your folders.")
                },
            )
        }
        many => say(channel, reply::which_file(name, many)),
    }
}

/// Where the sealed to-do list is kept.
fn tasks_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|dir| dir.join("tasks.dpapi"))
        .map_err(|e| e.to_string())
}

/// Loads the to-do list, lets `change` work on it, and saves it again if
/// `change` says it changed, all under one lock.
fn update_tasks<T>(
    app: &AppHandle,
    state: &AssistantState,
    change: impl FnOnce(&mut tasks::TaskList) -> (T, bool),
) -> Result<T, String> {
    let _one_at_a_time = state.tasks_lock.lock().unwrap_or_else(|e| e.into_inner());
    let path = tasks_path(app)?;
    let fail = |e: std::io::Error| format!("I couldn't open your to-do list: {e}");
    let mut list = tasks::load(&path).map_err(fail)?;
    let (result, changed) = change(&mut list);
    if changed {
        tasks::save(&path, &mut list, chrono::Local::now().timestamp()).map_err(fail)?;
    }
    Ok(result)
}

/// "Remind me to call mom at 5pm", "add milk to my list".
fn add_task_tool(
    app: &AppHandle,
    state: &AssistantState,
    text: &str,
    when_phrase: Option<&str>,
    channel: Sink<'_>,
) {
    let now = chrono::Local::now();
    let parsed = match when_phrase {
        Some(phrase) => match when::parse_phrase(phrase, now.naive_local()) {
            Some(Ok(due)) => Ok(when::Parsed {
                task: text.to_string(),
                due: Some(due),
            }),
            Some(Err(err)) => Err(err),
            None => {
                return say(
                    channel,
                    format!(
                        "I couldn't understand the time \"{}\". Try \"at 5pm\", \"in 20 minutes\" or \"tomorrow at 9am\".",
                        reply::safe_name(phrase)
                    ),
                )
            }
        },
        None => when::split(text, now.naive_local()),
    };
    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(when::WhenError::Passed) => {
            return say(
                channel,
                "That time has already passed. Try a later time, or \"tomorrow\".",
            )
        }
    };
    let due_unix = parsed.due.and_then(tasks::to_unix);
    let added = update_tasks(app, state, |list| {
        let result = list.add(&parsed.task, due_unix, now.timestamp());
        let changed = result.is_ok();
        (result, changed)
    });
    match added {
        Ok(Ok(task)) => say(channel, tasks::added_text(&task, now)),
        Ok(Err(tasks::AddError::Empty)) => say(channel, "What should I add to your list?"),
        Ok(Err(tasks::AddError::Full)) => say(
            channel,
            "Your list is full. Tick a few things off first, then I'll add more.",
        ),
        Err(err) => say(channel, err),
    }
}

/// "Mark milk as done" (or, with `remove`, "remove milk from my list").
fn finish_task_tool(
    app: &AppHandle,
    state: &AssistantState,
    name: &str,
    remove: bool,
    channel: Sink<'_>,
) {
    let now = chrono::Local::now().timestamp();
    let result = update_tasks(app, state, |list| {
        let found: Vec<tasks::Task> = list.find(name).into_iter().cloned().collect();
        match found.as_slice() {
            [task] => {
                let done = if remove {
                    list.remove(task.id, now)
                } else {
                    list.complete(task.id, now)
                };
                let text = reply::safe_name(&task.text);
                let message = match (done, remove) {
                    (true, false) => format!("Nice! I ticked off \"{text}\"."),
                    (true, true) => format!("Removed \"{text}\" from your list."),
                    (false, _) => format!("\"{text}\" is already off your list."),
                };
                (message, done)
            }
            [] => (
                format!(
                    "I don't see \"{}\" on your list. Say \"what's on my list?\" to see it.",
                    reply::safe_name(name)
                ),
                false,
            ),
            many => (tasks::which_text(&many.iter().collect::<Vec<_>>()), false),
        }
    });
    match result {
        Ok(message) | Err(message) => say(channel, message),
    }
}

/// How often reminders are checked.
const REMINDER_CHECK: Duration = Duration::from_secs(15);

/// Shows reminders when they are due: Cubby comes out of hiding and says it.
/// Reminders missed while the PC was off are shown at start-up.
pub fn start_reminders(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        loop {
            let state = app.state::<AssistantState>();
            let now = chrono::Local::now().timestamp();
            let due = update_tasks(&app, &state, |list| {
                let due = list.take_due(now);
                let changed = !due.is_empty();
                (due, changed)
            });
            for task in due.unwrap_or_default() {
                if let Some(window) = app.get_webview_window(crate::cubby_window::CUBBY_LABEL) {
                    let _ = window.show();
                }
                notify(&app, &format!("Reminder: {}", reply::safe_name(&task.text)));
            }
            std::thread::sleep(REMINDER_CHECK);
        }
    });
}

/// How long to wait for an app to close before saying it is asking
/// something (such as whether to save).
const CLOSE_WAIT: Duration = Duration::from_millis(2500);

/// "Close Notepad": asks the app's windows to close, as their X buttons do,
/// so an app with unsaved work can ask to save. Never forces anything.
fn close_app_tool(name: &str, channel: Sink<'_>) {
    use custodian_winsec::apps::{is_open, open_windows, request_close};
    if apps::means_everything(name) {
        return say(
            channel,
            "I close one app at a time, so nothing gets lost by surprise. Which one?",
        );
    }
    let found = apps::find(&open_windows(), name);
    let app = match found.as_slice() {
        [] => {
            return say(
                channel,
                format!(
                    "I don't see an open app called \"{}\". Say \"what's open?\" to see your apps.",
                    reply::safe_name(name)
                ),
            )
        }
        [app] => app,
        many => {
            let names: Vec<String> = many.iter().map(|a| reply::safe_name(&a.name)).collect();
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            return say(
                channel,
                format!("Which one do you mean: {}?", reply::join_list(&names)),
            );
        }
    };
    let asked: Vec<isize> = app
        .windows
        .iter()
        .filter(|w| request_close(w.handle))
        .map(|w| w.handle)
        .collect();
    if asked.is_empty() {
        return say(
            channel,
            format!(
                "I couldn't ask {} to close. It may be running as administrator, so please close it yourself.",
                reply::safe_name(&app.name)
            ),
        );
    }
    let deadline = Instant::now() + CLOSE_WAIT;
    while Instant::now() < deadline && asked.iter().any(|h| is_open(*h)) {
        std::thread::sleep(Duration::from_millis(100));
    }
    let still_open = asked.iter().filter(|h| is_open(**h)).count();
    say(
        channel,
        reply::closed_summary(&app.name, asked.len(), still_open),
    );
}

/// "Empty the safe place": frees the space of every copy now, after the
/// person confirms with Windows Hello (or the native dialog), because it
/// removes the easy way back. Each copy is checked against its original
/// first, and any that no longer match stay.
fn empty_safe_place_tool(app: &AppHandle, channel: Sink<'_>) {
    let q = match quarantine(app) {
        Ok(q) => q,
        Err(err) => return say(channel, format!("I couldn't open my safe place: {err}")),
    };
    let (files, total) = q.waiting();
    if files == 0 {
        return say(channel, "My safe place is already empty.");
    }
    if !confirm_with_person(
        app,
        "Empty Cubby's safe place",
        &reply::empty_question(files, total),
    ) {
        return say(
            channel,
            "Okay, everything stays in my safe place, and you can still undo.",
        );
    }
    emit(
        channel,
        ChatEvent::Status {
            text: "Checking each copy against its original…".into(),
        },
    );
    match q.purge_now(now_unix()) {
        Ok(outcome) => {
            let note = record_purge(app, &outcome, "person");
            say(
                channel,
                with_note(reply::purge_summary(&outcome, true), note),
            );
        }
        Err(err) => say(channel, format!("I couldn't empty my safe place: {err}")),
    }
}

/// Records a purge in the history ledger.
fn record_purge(
    app: &AppHandle,
    outcome: &custodian_core::cleanup::PurgeOutcome,
    by: &str,
) -> Option<String> {
    record_history(
        app,
        json!({
            "kind": "purge",
            "by": by,
            "purged": outcome.purged,
            "bytes": outcome.bytes,
            "kept": outcome.kept.len(),
            "at": now_unix(),
        }),
    )
}

/// First automatic check of the safe place after start-up, then how often.
const KEEPER_FIRST_CHECK: Duration = Duration::from_secs(2 * 60);
const KEEPER_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Event that shows a short, unprompted message from Cubby.
pub const NOTICE_EVENT: &str = "cubby-notice";

/// Shows `text` in Cubby's speech bubble and the chat. Notices never name
/// files, because they can appear while the screen is shared.
pub fn notify(app: &AppHandle, text: &str) {
    use tauri::Emitter;
    let _ = app.emit(NOTICE_EVENT, text);
}

/// Frees the space of copies whose 30 days have passed, in the background:
/// shortly after start-up and every few hours after that.
pub fn start_safe_place_keeper(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(KEEPER_FIRST_CHECK);
        // Copies kept back are mentioned once per run, not every check.
        let mut mentioned_kept = 0;
        loop {
            if let Ok(q) = quarantine(&app) {
                if let Ok(outcome) = q.purge_expired(now_unix()) {
                    if outcome.purged > 0 || outcome.kept.len() > mentioned_kept {
                        let note = record_purge(&app, &outcome, "schedule");
                        let mut report = outcome.clone();
                        if report.kept.len() <= mentioned_kept {
                            report.kept.clear();
                        }
                        mentioned_kept = mentioned_kept.max(outcome.kept.len());
                        let text = with_note(reply::purge_summary(&report, false), note);
                        if !text.is_empty() {
                            notify(&app, &text);
                        }
                    }
                }
            }
            std::thread::sleep(KEEPER_INTERVAL);
        }
    });
}

/// Runs `work` for the language model: the text it shows becomes result
/// blocks in the chat, and the model is told what was shown (or `work`'s own
/// note, e.g. "waiting for the person").
fn as_model_result(channel: Sink<'_>, work: impl FnOnce(Sink<'_>) -> Option<String>) -> String {
    let shown = RefCell::new(String::new());
    let relabel = |event: ChatEvent| match event {
        ChatEvent::Text { text } => {
            {
                let mut shown = shown.borrow_mut();
                if !shown.is_empty() {
                    shown.push_str("\n\n");
                }
                shown.push_str(&text);
            }
            emit(channel, ChatEvent::Result { text });
        }
        other => emit(channel, other),
    };
    if let Some(note) = work(&relabel) {
        return note;
    }
    let shown = shown.into_inner();
    if shown.is_empty() {
        "Done.".into()
    } else {
        format!("Shown to the person: {shown}")
    }
}

/// Runs a tool the language model asked for, after checking its arguments.
fn run_model_tool(
    app: &AppHandle,
    state: &AssistantState,
    name: &str,
    args: &serde_json::Value,
    channel: Sink<'_>,
) -> String {
    let call = match ToolCall::parse(name, args) {
        Ok(call) => call,
        Err(problem) => return format!("That tool call was refused: {problem}."),
    };
    as_model_result(channel, |sink| {
        execute(app, state, call, Origin::Model, sink)
    })
}

fn find_duplicates_tool(app: &AppHandle, state: &AssistantState, scope: &Scope, channel: Sink<'_>) {
    let folders = resolve_scope(app, scope);
    if let Some(plan) = run_duplicates(&folders, &state.cancel, channel) {
        state
            .plans
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(plan.id.clone(), PendingPlan::Duplicates(plan));
    }
}

/// "Clean temp files": proposes deleting temporary files older than a week.
fn clean_temp_tool(state: &AssistantState, channel: Sink<'_>) {
    let Some(root) = temp::user_temp_dir() else {
        return say(
            channel,
            "I couldn't find your temporary files folder in its usual place, so I'll leave it alone.",
        );
    };
    emit(
        channel,
        ChatEvent::Status {
            text: "Looking for old temporary files…".into(),
        },
    );
    let plan = temp::scan(&root, SystemTime::now(), &state.cancel);
    if state.cancel.load(Ordering::Relaxed) {
        return say(channel, STOPPED);
    }
    if plan.items.is_empty() {
        return say(
            channel,
            "Your temporary files are already tidy: nothing there is older than a week.",
        );
    }
    say(
        channel,
        reply::temp_found(plan.items.len(), plan.total_bytes()),
    );
    let view = PlanView {
        id: plan.id.clone(),
        risk_level: "medium",
        summary: reply::temp_plan_summary(plan.items.len(), plan.total_bytes()),
        total_bytes: plan.total_bytes(),
        file_count: plan.items.len(),
        operations: plan
            .items
            .iter()
            .take(MAX_VIEW_OPERATIONS)
            .map(|i| OperationView {
                path: reply::safe_path(&i.path),
                keep: String::new(),
                action: "delete",
                size_bytes: i.size,
            })
            .collect(),
    };
    emit(channel, ChatEvent::Plan { plan: view });
    state
        .plans
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(plan.id.clone(), PendingPlan::Temp(plan));
}

/// Scans `folders` for duplicates, reports them, and proposes a clean-up plan
/// (returned so the caller can keep it for approval).
fn run_duplicates(
    folders: &[(KnownFolder, PathBuf)],
    cancel: &AtomicBool,
    channel: Sink<'_>,
) -> Option<CleanupPlan> {
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
        cancel,
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
    if cancel.load(Ordering::Relaxed) {
        // A partial scan could miss copies; propose nothing from it.
        say(channel, STOPPED);
        return None;
    }

    // Copies in OneDrive (or another synced folder) are never proposed:
    // removing them would reach the cloud and other devices too (K-14).
    let synced = custodian_winsec::sync_roots::sync_roots();
    let (mut items, left) = duplicate_plan_items(&report.groups, &|path| {
        custodian_winsec::sync_roots::sync_root_of(path, &synced).map(|r| r.provider.clone())
    });
    items.truncate(MAX_PLAN_ITEMS);
    let mut text = reply::duplicates_summary(&report, &labels);
    if left.files > 0 {
        text.push_str(&reply::left_out_note(&left));
    }
    if items.is_empty() {
        say(channel, text);
        return None;
    }
    text.push_str(
        "

I'll keep the original of each and move the extra copies to my safe place. Review the plan below.",
    );
    say(channel, text);

    let plan = CleanupPlan::new(items);
    let view = PlanView {
        id: plan.id.clone(),
        risk_level: "low",
        summary: format!(
            "Move {} duplicate {} ({}) to Cubby's safe place. {}",
            reply::count(plan.items.len()),
            reply::plural(plan.items.len(), "copy", "copies"),
            reply::bytes(plan.total_bytes()),
            safe_place_terms()
        ),
        total_bytes: plan.total_bytes(),
        file_count: plan.items.len(),
        operations: plan
            .items
            .iter()
            .take(MAX_VIEW_OPERATIONS)
            .map(|i| OperationView {
                path: reply::safe_path(&i.path),
                keep: reply::safe_path(&i.keep),
                action: "quarantine",
                size_bytes: i.size,
            })
            .collect(),
    };
    emit(channel, ChatEvent::Plan { plan: view });
    Some(plan)
}

fn run_largest(folders: &[(KnownFolder, PathBuf)], cancel: &AtomicBool, channel: Sink<'_>) {
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
    let (files, stats) = largest_files(&roots, 10, WalkLimits::default(), cancel, &mut |n| {
        if throttle.ready() {
            emit(
                channel,
                ChatEvent::Status {
                    text: format!("Scanning… {} files", reply::count(n)),
                },
            );
        }
    });
    if cancel.load(Ordering::Relaxed) {
        say(channel, STOPPED);
        return;
    }
    say(
        channel,
        reply::largest_summary(&files, &labels, stats.files_seen),
    );
}

/// "Where's my resume?": files whose names contain the words, newest first.
fn run_find(
    folders: &[(KnownFolder, PathBuf)],
    name: &str,
    cancel: &AtomicBool,
    channel: Sink<'_>,
) {
    if folders.is_empty() {
        say(channel, "I couldn't find those folders on this PC.");
        return;
    }
    let labels: Vec<&str> = folders.iter().map(|(f, _)| f.label()).collect();
    emit(
        channel,
        ChatEvent::Status {
            text: format!(
                "Looking for \"{}\" in {}…",
                reply::safe_name(name),
                reply::join_list(&labels)
            ),
        },
    );
    let roots: Vec<PathBuf> = folders.iter().map(|(_, p)| p.clone()).collect();
    let mut throttle = Throttle(Instant::now());
    let (files, stats) = find_by_name(
        &roots,
        name,
        MAX_FOUND_FILES,
        WalkLimits::default(),
        cancel,
        &mut |n| {
            if throttle.ready() {
                emit(
                    channel,
                    ChatEvent::Status {
                        text: format!("Searching… {} files", reply::count(n)),
                    },
                );
            }
        },
    );
    if cancel.load(Ordering::Relaxed) {
        say(channel, STOPPED);
        return;
    }
    say(
        channel,
        reply::find_summary(name, &files, &labels, stats, SystemTime::now()),
    );
}

/// The PC's fixed drives: (name, free bytes, total bytes).
fn fixed_drives() -> Vec<(String, u64, u64)> {
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
    drives
}

fn disk_space_tool(channel: Sink<'_>) {
    say(channel, reply::disk_summary(&fixed_drives()));
}

/// Measures everything the check-up looks at. Read-only.
fn gather_facts(app: &AppHandle, state: &AssistantState) -> advice::Facts {
    let now = chrono::Local::now();
    let temp = temp::user_temp_dir().map(|root| {
        let plan = temp::scan(&root, SystemTime::now(), &state.cancel);
        (plan.items.len(), plan.total_bytes())
    });
    let old_installers = folder_path(app, KnownFolder::Downloads)
        .map(|dir| advice::old_installers(&dir, SystemTime::now()));
    let due_today = update_tasks(app, state, |list| {
        let today = now.date_naive();
        let due: Vec<(String, String)> = list
            .open()
            .into_iter()
            .filter_map(|t| {
                let due =
                    chrono::DateTime::from_timestamp(t.due_unix?, 0)?.with_timezone(&chrono::Local);
                (due >= now && due.date_naive() == today)
                    .then(|| (t.text.clone(), due.format("%-I:%M %p").to_string()))
            })
            .collect();
        (due, false)
    })
    .unwrap_or_default();
    advice::Facts {
        drives: fixed_drives(),
        temp,
        old_installers,
        recycle_bin: custodian_winsec::shell::recycle_bin().map(|b| (b.items, b.bytes)),
        safe_place: quarantine(app).map(|q| q.waiting()).unwrap_or((0, 0)),
        system: Some(system::snapshot()),
        due_today,
    }
}

/// "Any suggestions?": a quick check-up and a short list of what to do.
fn suggestions_tool(app: &AppHandle, state: &AssistantState, channel: Sink<'_>) {
    emit(
        channel,
        ChatEvent::Status {
            text: "Giving your PC a quick check-up…".into(),
        },
    );
    let facts = gather_facts(app, state);
    if state.cancel.load(Ordering::Relaxed) {
        return say(channel, STOPPED);
    }
    say(channel, advice::advice_text(&advice::advise(&facts)));
}

/// How often Cubby considers an unprompted nudge, and the least time
/// between any two of them (§14: Cubby never nags).
const NUDGE_CHECK: Duration = Duration::from_secs(10 * 60);
const NUDGE_GAP: Duration = Duration::from_secs(60 * 60);

/// Nudges about a drive about to fill or a dying battery, rarely, and never
/// while the person is presenting, gaming or away.
pub fn start_nudges(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3 * 60));
        let mut last_any: Option<Instant> = None;
        let mut last: HashMap<advice::NudgeKind, Instant> = HashMap::new();
        loop {
            let available = custodian_winsec::shell::attention()
                == custodian_winsec::shell::Attention::Available;
            let gap_ok = last_any.is_none_or(|t| t.elapsed() >= NUDGE_GAP);
            if available && gap_ok {
                let battery = custodian_winsec::power::battery().map(|b| (b.percent, b.plugged_in));
                if let Some((kind, text)) = advice::nudge(&fixed_drives(), battery) {
                    if last
                        .get(&kind)
                        .is_none_or(|t| t.elapsed() >= kind.quiet_for())
                    {
                        notify(&app, &text);
                        let now = Instant::now();
                        last.insert(kind, now);
                        last_any = Some(now);
                    }
                }
            }
            std::thread::sleep(NUDGE_CHECK);
        }
    });
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

fn open_app_tool(app: &AppHandle, kind: App, channel: Sink<'_>) {
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

fn open_folder_tool(app: &AppHandle, folder: KnownFolder, channel: Sink<'_>) {
    match folder_path(app, folder) {
        Some(path) => match app
            .opener()
            .open_path(path.display().to_string(), None::<&str>)
        {
            Ok(()) => say(channel, format!("Opening your {} folder.", folder.label())),
            Err(err) => say(
                channel,
                format!("I couldn't open {}: {err}", folder.label()),
            ),
        },
        None => say(
            channel,
            format!("I couldn't find a {} folder on this PC.", folder.label()),
        ),
    }
}

fn open_website_tool(app: &AppHandle, name: &str, url: &str, channel: Sink<'_>) {
    match app.opener().open_url(url, None::<&str>) {
        Ok(()) => say(
            channel,
            format!("Opening {} in your browser.", reply::title_case(name)),
        ),
        Err(err) => say(channel, format!("I couldn't open {url}: {err}")),
    }
}

fn web_search_tool(app: &AppHandle, query: &str, channel: Sink<'_>) {
    let url = format!("https://www.google.com/search?q={}", encode_query(query));
    match app.opener().open_url(&url, None::<&str>) {
        Ok(()) => say(
            channel,
            format!(
                "Searching the web for \"{}\" in your browser.",
                reply::safe_name(query)
            ),
        ),
        Err(err) => say(channel, format!("I couldn't open your browser: {err}")),
    }
}

fn undo_tool(app: &AppHandle, channel: Sink<'_>) {
    match quarantine(app).and_then(|q| q.undo_last().map_err(|e| e.to_string())) {
        Ok(outcome) => {
            let note = record_history(
                app,
                json!({
                    "kind": "undo",
                    "plan": outcome.plan_id,
                    "restored": outcome.restored,
                    "skipped": outcome.skipped.len(),
                    "at": now_unix(),
                }),
            );
            say(channel, with_note(reply::undo_summary(&outcome), note));
        }
        Err(err) => say(channel, reply::undo_error(&err)),
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

/// Starts the agent unless it is already running. Returns whether it had
/// to be started, or why it could not be.
fn ensure_agent(app: &AppHandle, agent: &mut Option<AgentProcess>) -> Result<bool, String> {
    if agent.as_mut().is_some_and(|a| !a.is_alive()) {
        *agent = None;
    }
    if agent.is_some() {
        return Ok(false);
    }
    let entry = agent_entry(app)
        .ok_or("the agent isn't built yet (run `npm run build` in the repository)")?;
    let node = std::env::var("CUSTODIAN_NODE").unwrap_or_else(|_| "node".into());
    let models = models_dir(app);
    #[cfg(windows)]
    if let Some(sandboxed) = spawn_sandboxed_agent(&node, &entry, models.as_deref()) {
        *LAST_LAUNCH_SANDBOXED
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(sandboxed.sandboxed());
        *agent = Some(sandboxed);
        return Ok(true);
    }
    let started =
        AgentProcess::spawn(&node, &entry, models.as_deref()).map_err(|e| e.to_string())?;
    *LAST_LAUNCH_SANDBOXED
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(started.sandboxed());
    *agent = Some(started);
    Ok(true)
}

/// The AppContainer the agent runs in.
#[cfg(windows)]
const AGENT_CONTAINER: &str = "custodian.agent";

/// Why the agent could not run in its sandbox, once it has tried and failed.
static SANDBOX_PROBLEM: Mutex<Option<String>> = Mutex::new(None);

/// Whether the agent ran in its sandbox when it last started (`None`: not
/// started since Custodian opened).
static LAST_LAUNCH_SANDBOXED: Mutex<Option<bool>> = Mutex::new(None);

/// Cubby's protections as they stand right now, for "is my data safe?".
fn protections(app: &AppHandle) -> reply::Protections {
    let ai_sandboxed = *LAST_LAUNCH_SANDBOXED
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let sandbox_problem = SANDBOX_PROBLEM
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let history_intact = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())
        .and_then(|dir| history_ledger::check(&dir.join("ledger")))
        .is_ok_and(|status| status == history_ledger::LedgerStatus::Intact);
    let mut synced: Vec<String> = custodian_winsec::sync_roots::sync_roots()
        .into_iter()
        .map(|root| root.provider)
        .collect();
    synced.dedup();
    reply::Protections {
        ai_sandboxed,
        sandbox_problem,
        hello: custodian_winsec::hello::available(),
        history_intact,
        hidden_from_capture: !crate::capture::is_allowed(),
        synced,
    }
}

/// The agent's sandbox, prepared once: the container exists and may read
/// what the agent needs. Later calls wait for the first to finish.
#[cfg(windows)]
fn agent_sandbox(
    node: &Path,
    entry: &Path,
    models: Option<&Path>,
) -> Result<Arc<custodian_winsec::appcontainer::Sandbox>, String> {
    static SANDBOX: Mutex<Option<Result<Arc<custodian_winsec::appcontainer::Sandbox>, String>>> =
        Mutex::new(None);
    let mut slot = SANDBOX.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(result) = slot.as_ref() {
        return result.clone();
    }
    let result = custodian_winsec::appcontainer::Sandbox::open(AGENT_CONTAINER)
        .map_err(|e| e.to_string())
        .map(|sandbox| {
            for root in crate::agent::sandbox_read_roots(node, entry, models) {
                // A folder Windows already lets every app read (an install
                // under Program Files) refuses changes; that is fine.
                if let Err(err) = sandbox.grant_read(&root) {
                    eprintln!(
                        "[desktop] sandbox read access for {}: {err}",
                        root.display()
                    );
                }
            }
            Arc::new(sandbox)
        });
    *slot = Some(result.clone());
    result
}

/// Starts the agent inside its sandbox (no network, none of the person's
/// files) and checks that it can load its model there. `None` means it could
/// not, and the agent should start normally; the reason is logged, and the
/// sandbox is not tried again until Custodian restarts.
#[cfg(windows)]
fn spawn_sandboxed_agent(node: &str, entry: &Path, models: Option<&Path>) -> Option<AgentProcess> {
    static GAVE_UP: AtomicBool = AtomicBool::new(false);
    if GAVE_UP.load(Ordering::Relaxed)
        || std::env::var_os("CUSTODIAN_AGENT_SANDBOX").is_none_or(|v| v != "1")
    {
        return None;
    }
    let attempt = || -> Result<AgentProcess, String> {
        let node = crate::agent::resolve_node(node).ok_or("node was not found on the PATH")?;
        let sandbox = agent_sandbox(&node, entry, models)?;
        let mut agent = AgentProcess::spawn_sandboxed(&sandbox, &node, entry, models)
            .map_err(|e| e.to_string())?;
        match agent.ping().map_err(|e| e.to_string())? {
            None => Ok(agent),
            Some(problem) => Err(problem),
        }
    };
    match attempt() {
        Ok(agent) => {
            eprintln!(
                "[desktop] the agent runs in its sandbox (no network, no access to your files)"
            );
            Some(agent)
        }
        Err(reason) => {
            eprintln!(
                "[desktop] the agent could not run in its sandbox, so it runs normally: {reason}"
            );
            *SANDBOX_PROBLEM.lock().unwrap_or_else(|e| e.into_inner()) = Some(reason);
            GAVE_UP.store(true, Ordering::Relaxed);
            None
        }
    }
}

/// Starts the agent ahead of time (when the chat panel opens), so the model
/// is loaded by the time the first question arrives. Failures are reported
/// later, on that question.
pub fn prewarm(app: &AppHandle) {
    let state = app.state::<AssistantState>();
    let mut agent = state.agent.lock().unwrap_or_else(|e| e.into_inner());
    if ensure_agent(app, &mut agent).is_ok() {
        mark_agent_used(&state);
    }
}

/// Answers open-ended messages with the local model.
fn ask_model(app: &AppHandle, state: &AssistantState, message: &str, channel: Sink<'_>) {
    let mut agent = state.agent.lock().unwrap_or_else(|e| e.into_inner());
    match ensure_agent(app, &mut agent) {
        Ok(true) => emit(
            channel,
            ChatEvent::Status {
                text: "Waking up my brain… the first answer takes a moment.".into(),
            },
        ),
        Ok(false) => {}
        Err(err) => {
            say(channel, reply::no_brain(&err));
            return;
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
    let result = process.chat(
        message,
        &state.cancel,
        &mut |text| say(channel, text),
        &mut |name, args| run_model_tool(app, state, name, args, channel),
    );
    mark_agent_used(state);
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
    let parsed = intent::parse(message);
    match parsed {
        Intent::CloseCubby => {
            say(
                channel,
                "I can't close myself, but you can hide me from the tray icon, or say \"go to sleep\".",
            );
            return;
        }
        Intent::Sleep => {
            say(
                channel,
                "Okay, taking a little nap. Move your mouse near me to wake me up.",
            );
            emit(
                channel,
                ChatEvent::Action {
                    action: "sleep".into(),
                },
            );
            return;
        }
        _ => {}
    }

    // A settings request is resolved against the catalogue before it can
    // become a call, so nothing outside that catalogue is ever changed.
    if let Intent::Settings(words) = &parsed {
        if resolve_settings_request(app, state, words, channel) {
            return;
        }
    }

    if let Some(call) = ToolCall::from_intent(&parsed) {
        execute(app, state, call, Origin::Person, channel);
        return;
    }

    ask_model(app, state, message, channel);
}

#[tauri::command]
pub async fn send_chat_message(
    app: AppHandle,
    message: String,
    channel: Channel<ChatEvent>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        state.cancel.store(false, Ordering::Relaxed);
        // A closed chat panel is not an error.
        let sink = |event: ChatEvent| {
            let _ = channel.send(event);
        };
        respond(&app, &state, &message, &sink);
    })
    .await
    .map_err(|e| e.to_string())
}

/// The Stop button: ends the scan or answer in progress.
#[tauri::command]
pub fn cancel_chat(state: tauri::State<'_, AssistantState>) {
    state.cancel.store(true, Ordering::Relaxed);
}

/// The person's answer to a card. Only the card's id comes from the
/// webview; what runs is the action Cubby stored when it made the card.
#[tauri::command]
pub async fn decide_action(
    app: AppHandle,
    action_id: String,
    allow: bool,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        let call = state
            .actions
            .take(&action_id, Instant::now())
            .ok_or_else(|| CARD_GONE.to_string())?;
        if !allow {
            return Ok(actions::declined_text(&call));
        }
        state.cancel.store(false, Ordering::Relaxed);
        let said = RefCell::new(Vec::new());
        let sink = |event: ChatEvent| {
            if let ChatEvent::Text { text } = event {
                said.borrow_mut().push(text);
            }
        };
        execute(&app, &state, call, Origin::Approved, &sink);
        Ok(said.into_inner().join("\n\n"))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Executes a clean-up plan that Cubby proposed, after the person confirms it
/// with Windows Hello (or a native dialog where Hello is not set up). Only plans created by the assistant itself can be
/// approved; the webview never supplies paths and cannot approve on its own.
#[tauri::command]
pub async fn approve_plan(app: AppHandle, plan_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        let approvals = approvals(&app, &state)?;
        let outcome = approve_and_run(
            &state.plans,
            &plan_id,
            &approvals,
            &quarantine(&app)?,
            |question| confirm_with_person(&app, "Approve clean-up", question),
        )?;
        Ok(match outcome {
            PlanOutcome::Cleanup(outcome) => {
                let note = record_history(
                    &app,
                    json!({
                        "kind": "cleanup",
                        "plan": outcome.plan_id,
                        "moved": outcome.moved,
                        "bytes": outcome.bytes,
                        "skipped": outcome.skipped.len(),
                        "at": now_unix(),
                    }),
                );
                with_note(reply::cleanup_summary(&outcome), note)
            }
            PlanOutcome::Temp(outcome) => {
                let note = record_history(
                    &app,
                    json!({
                        "kind": "temp_cleanup",
                        "deleted": outcome.deleted,
                        "bytes": outcome.bytes,
                        "skipped": outcome.skipped,
                        "at": now_unix(),
                    }),
                );
                with_note(reply::temp_summary(&outcome), note)
            }
        })
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

    /// A settings change waits for the person's click even when they asked for
    /// it, and the click must not ask again — otherwise approving a card would
    /// raise another card, for ever.
    #[test]
    fn a_settings_card_is_asked_for_once_and_the_click_ends_it() {
        let change = ToolCall::ChangeSetting {
            setting: "dark_mode_apps".into(),
            value: "dark".into(),
        };
        assert!(
            needs_card(&change, Origin::Person),
            "the person is asked too"
        );
        assert!(needs_card(&change, Origin::Model));
        assert!(
            !needs_card(&change, Origin::Approved),
            "asking again would loop"
        );

        // Everything else keeps its old behaviour: the person's own request
        // runs at once, the model's waits.
        assert!(!needs_card(&ToolCall::CheckDiskSpace, Origin::Person));
        assert!(!needs_card(
            &ToolCall::WebSearch("flights".into()),
            Origin::Person
        ));
        assert!(needs_card(
            &ToolCall::WebSearch("flights".into()),
            Origin::Model
        ));
        assert!(!needs_card(
            &ToolCall::WebSearch("flights".into()),
            Origin::Approved
        ));
        // Putting a setting back is the person's own request: no card either.
        assert!(!needs_card(&ToolCall::RevertSettingChange, Origin::Person));
    }

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
        let plan = run_duplicates(&folders, &AtomicBool::new(false), &sink)
            .expect("a plan for the duplicates");
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

        // 2. Approve: nothing moves until the person confirms; then the extra
        //    copies move to the safe place and the original stays.
        let quarantine = Quarantine::new(dir.path().join("quarantine"));
        let approvals = ApprovalService::with_key(custodian_approval::ApprovalKey::generate());
        let plans = Mutex::new(HashMap::from([(
            plan.id.clone(),
            PendingPlan::Duplicates(plan.clone()),
        )]));
        let declined = approve_and_run(&plans, &plan.id, &approvals, &quarantine, |q| {
            assert!(q.contains("Move 2 duplicate copies"), "{q}");
            false
        });
        assert_eq!(declined.unwrap_err(), NOT_CONFIRMED);
        assert!(view
            .operations
            .iter()
            .all(|op| PathBuf::from(&op.path).exists()));
        assert!(
            plans.lock().unwrap().contains_key(&plan.id),
            "still pending"
        );

        // "Not now" while the dialog is open wins, whatever the dialog says.
        let cancelled = approve_and_run(&plans, &plan.id, &approvals, &quarantine, |_| {
            plans.lock().unwrap().remove(&plan.id);
            true
        });
        assert_eq!(cancelled.unwrap_err(), PLAN_GONE);
        assert!(view
            .operations
            .iter()
            .all(|op| PathBuf::from(&op.path).exists()));

        plans
            .lock()
            .unwrap()
            .insert(plan.id.clone(), PendingPlan::Duplicates(plan.clone()));
        let PlanOutcome::Cleanup(outcome) =
            approve_and_run(&plans, &plan.id, &approvals, &quarantine, |_| true).unwrap()
        else {
            panic!("a duplicate plan gives a clean-up outcome");
        };
        assert_eq!(
            approve_and_run(&plans, &plan.id, &approvals, &quarantine, |_| true).unwrap_err(),
            PLAN_GONE
        );
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
        run_largest(
            &[(KnownFolder::Videos, dir.path().to_path_buf())],
            &AtomicBool::new(false),
            &sink,
        );
        let text = match events.into_inner().pop() {
            Some(ChatEvent::Text { text }) => text,
            other => panic!("expected text, got {other:?}"),
        };
        let movie = text.find("movie.mp4").unwrap();
        let note = text.find("note.txt").unwrap();
        assert!(movie < note);
    }

    /// The text events a run produced, joined.
    fn texts(events: &[ChatEvent]) -> String {
        events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn finds_files_by_name_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("Documents");
        fs::create_dir_all(docs.join("old")).unwrap();
        fs::write(docs.join("Resume 2026.pdf"), b"cv").unwrap();
        fs::write(docs.join("old/resume.docx"), b"cv").unwrap();
        fs::write(docs.join("taxes.xlsx"), b"tax").unwrap();
        let events = RefCell::new(Vec::new());
        let sink = |e: ChatEvent| events.borrow_mut().push(e);
        run_find(
            &[(KnownFolder::Documents, docs.clone())],
            "resume",
            &AtomicBool::new(false),
            &sink,
        );
        let text = texts(&events.into_inner());
        assert!(
            text.starts_with("I found 2 files with \"resume\""),
            "{text}"
        );
        assert!(text.contains("Resume 2026.pdf — in Documents"), "{text}");
        assert!(text.contains("resume.docx — in old"), "{text}");
        assert!(!text.contains("taxes"), "{text}");
    }

    #[test]
    fn a_stopped_scan_proposes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.jpg"), vec![7u8; 4096]).unwrap();
        fs::write(dir.path().join("b.jpg"), vec![7u8; 4096]).unwrap();
        let events = RefCell::new(Vec::new());
        let sink = |e: ChatEvent| events.borrow_mut().push(e);
        let stopped = AtomicBool::new(true);
        let plan = run_duplicates(
            &[(KnownFolder::Pictures, dir.path().to_path_buf())],
            &stopped,
            &sink,
        );
        assert!(plan.is_none());
        let events = events.into_inner();
        assert_eq!(texts(&events), STOPPED);
        assert!(!events.iter().any(|e| matches!(e, ChatEvent::Plan { .. })));
    }

    #[test]
    fn the_model_sees_what_was_shown_as_a_separate_result() {
        let events = RefCell::new(Vec::new());
        let sink = |e: ChatEvent| events.borrow_mut().push(e);
        let told = as_model_result(&sink, |inner| {
            inner(ChatEvent::Status {
                text: "Scanning…".into(),
            });
            inner(ChatEvent::Text {
                text: "C: has 41 GB free.".into(),
            });
            None
        });
        assert_eq!(told, "Shown to the person: C: has 41 GB free.");
        let seen = events.borrow().clone();
        assert!(matches!(&seen[0], ChatEvent::Status { .. }));
        assert!(
            matches!(&seen[1], ChatEvent::Result { text } if text == "C: has 41 GB free."),
            "tool text is a result block, not model text"
        );
        assert!(!seen.iter().any(|e| matches!(e, ChatEvent::Text { .. })));

        // A waiting card tells the model so, instead of what was shown.
        let told = as_model_result(&sink, |_| Some("Waiting for the person".into()));
        assert_eq!(told, "Waiting for the person");
        assert_eq!(as_model_result(&sink, |_| None), "Done.");
    }

    #[test]
    fn unloads_the_model_only_after_ten_idle_minutes() {
        let now = Instant::now();
        let ago = |secs| now.checked_sub(Duration::from_secs(secs));
        let limit = AGENT_IDLE_LIMIT;
        assert!(
            !agent_is_idle(false, ago(3600), now, limit),
            "nothing loaded"
        );
        assert!(!agent_is_idle(true, ago(9 * 60), now, limit));
        assert!(agent_is_idle(true, ago(10 * 60), now, limit));
        assert!(agent_is_idle(true, None, now, limit), "never used");
    }

    #[test]
    fn encodes_search_queries() {
        assert_eq!(encode_query("cheap flights"), "cheap+flights");
        assert_eq!(encode_query("a&b=c"), "a%26b%3Dc");
    }
}
