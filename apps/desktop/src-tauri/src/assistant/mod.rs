#![allow(unused_imports, dead_code)]
//! Cubby's assistant: understands a chat message, runs the matching tool and
//! streams the answer back to the chat panel. Requests the rules don't
//! recognise go to the local language model through the agent process, which
//! may ask for the same tools (checked again in [`tools`]).

mod actions;
mod advice;
mod app_tasks;
mod apps;
mod capability;
mod file_ops;
pub mod intent;
mod language;
mod launcher;
pub mod privacy;
mod references;
mod reply;
pub mod routine;
mod settings;
mod system;
mod tasks;
mod tools;
mod when;
mod workflow;

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
    routine: Arc<custodian_core::routine::RoutinePolicy>,
    privacy: Arc<custodian_core::privacy::AccessPolicy>,
    app_privacy: custodian_core::app_authority::ApplicationAuthority,
    agent_policy_revision: std::sync::atomic::AtomicU64,
    agent_app_policy_revision: std::sync::atomic::AtomicU64,
    plans: Mutex<HashMap<String, PendingPlan>>,
    agent: Mutex<Option<AgentProcess>>,
    /// When the agent was last started or asked something.
    agent_used: Mutex<Option<Instant>>,
    approvals: Mutex<Option<Arc<ApprovalService>>>,
    /// Model requests waiting for the person's click.
    actions: PendingActions,
    app_tasks: app_tasks::Pending,
    /// Set by the Stop button; scans and the model check it and stop.
    cancel: AtomicBool,
    /// Stop invalidates requests which have not entered the worker pool yet.
    request_epoch: Arc<std::sync::atomic::AtomicU64>,
    request_admission: Mutex<()>,
    /// One reader or writer of the to-do list at a time.
    tasks_lock: Mutex<()>,
    /// Recent verified events, tool results, and card outcomes for conversation context projection (R14).
    history_context: Mutex<Vec<String>>,
    file_references: references::FileReferences,
    /// Task coordinator managing background tasks, lifecycle states and resource locks (M6 / F46).
    pub coordinator: custodian_core::coordinator::TaskCoordinator,
}

impl AssistantState {
    fn create_task(
        &self,
        description: String,
        initial: custodian_core::coordinator::TaskState,
        resources: &[PathBuf],
    ) -> Result<(String, Arc<AtomicBool>), String> {
        // Stop and task registration share a lock: a task cannot appear just
        // after cancel_all has walked the active task list.
        let _admission = self
            .request_admission
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if self.cancel.load(Ordering::Acquire) || !custodian_core::privacy::request_is_current() {
            return Err(STOPPED.into());
        }
        self.coordinator
            .create_task(description, initial, resources)
            .map_err(|e| e.to_string())
    }
    fn request_epoch(&self) -> u64 {
        self.request_epoch.load(Ordering::Acquire)
    }

    fn begin_request(&self, epoch: u64, privacy_revision: u64) -> bool {
        let _admission = self
            .request_admission
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if self.request_epoch() != epoch || self.privacy.revision() != privacy_revision {
            return false;
        }
        self.cancel.store(false, Ordering::Release);
        true
    }

    fn stop_requests(&self) {
        let _admission = self
            .request_admission
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.request_epoch.fetch_add(1, Ordering::AcqRel);
        self.cancel.store(true, Ordering::Release);
        self.app_tasks.clear();
        self.coordinator.cancel_all();
    }
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let coordinator = custodian_core::coordinator::TaskCoordinator::open(
            data_dir.join("background-tasks.dpapi"),
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            app_privacy: custodian_core::app_authority::ApplicationAuthority::open(
                data_dir.join("app-privacy.dpapi"),
            )
            .map_err(str::to_string)?,
            routine: Arc::new(
                custodian_core::routine::RoutinePolicy::open(data_dir.join("routine.dpapi"))
                    .map_err(|_| {
                        "Routine permissions could not be verified. Startup was stopped."
                            .to_string()
                    })?,
            ),
            coordinator,
            privacy: Arc::new(
                custodian_core::privacy::AccessPolicy::open(data_dir.join("privacy.dpapi"))
                    .map_err(|_| {
                        "Privacy rules could not be verified. Startup was stopped.".to_string()
                    })?,
            ),
            ..Self::default()
        })
    }
    /// Records a verified event or action outcome to the recent context projection.
    pub fn record_context(&self, entry: String) {
        if !custodian_core::privacy::request_is_current() {
            return;
        }
        if !custodian_core::privacy::request_is_current() {
            return;
        }
        if !self.privacy.allows_text(&entry) {
            return;
        }
        let mut ctx = self
            .history_context
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        ctx.push(entry);
        if ctx.len() > 20 {
            let excess = ctx.len() - 20;
            ctx.drain(0..excess);
        }
    }

    /// Returns the context projection formatted for prompt injection into the model, or empty string.
    pub fn get_context_projection(&self) -> String {
        if !custodian_core::privacy::request_is_current() {
            return String::new();
        }
        let ctx = self
            .history_context
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if ctx.is_empty() {
            return String::new();
        }
        format!(
            "<data>\nRecent PC activity and verified results:\n{}\n</data>\n\n",
            ctx.iter()
                .filter(|entry| self.privacy.allows_text(entry))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        )
    }

    /// Clears the context projection (for a fresh conversation).
    pub fn reset_context(&self) {
        let mut ctx = self
            .history_context
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        ctx.clear();
        self.file_references.clear();
    }

    fn invalidate_conversation(&self) {
        self.stop_requests();
        self.actions.clear();
        self.plans.lock().unwrap_or_else(|e| e.into_inner()).clear();
        self.reset_context();
    }
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
    capability::Contract::for_call(call).needs_approval(call, origin)
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
    match crate::paths::verification_folder(folder.label()) {
        Ok(Some(path)) => return path.is_dir().then_some(path),
        Ok(None) => {}
        Err(_) => return None,
    }
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

/// Deduplicates search roots:
/// - If a candidate root is a sub-path of an already included root, skip it.
/// - If a candidate root is a parent of an already included root, remove the narrower root and include the broader root (F18 / A16).
pub fn deduplicate_roots(candidates: Vec<(String, PathBuf)>) -> Vec<(String, PathBuf)> {
    let mut resolved: Vec<(String, PathBuf)> = Vec::new();
    for (label, path) in candidates {
        if resolved
            .iter()
            .any(|(_, existing)| path.starts_with(existing))
        {
            continue;
        }
        resolved.retain(|(_, existing)| !existing.starts_with(&path));
        resolved.push((label, path));
    }
    resolved
}

/// Existing folders or drives for a scope, with their labels.
fn resolve_scope(app: &AppHandle, scope: &Scope) -> Vec<(String, PathBuf)> {
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    match scope {
        Scope::UserFolders => {
            for folder in KnownFolder::ALL {
                if let Some(path) = folder_path(app, folder) {
                    candidates.push((folder.label().to_string(), path));
                }
            }
        }
        Scope::Folders(list) => {
            for folder in list {
                if let Some(path) = folder_path(app, *folder) {
                    candidates.push((folder.label().to_string(), path));
                }
            }
        }
        Scope::AllDrives => {
            for (mount, _, _) in fixed_drives() {
                let p = PathBuf::from(&mount);
                if custodian_core::privacy::check(&p).is_ok() && p.is_dir() {
                    candidates.push((mount, p));
                }
            }
        }
        Scope::Drive(d) => {
            let mut drive_str = d.clone();
            if !drive_str.ends_with('\\') && !drive_str.ends_with('/') {
                drive_str.push('\\');
            }
            let p = PathBuf::from(&drive_str);
            if custodian_core::privacy::check(&p).is_ok() && p.is_dir() {
                candidates.push((d.clone(), p));
            }
        }
        Scope::Custom(p) => {
            if custodian_core::privacy::check(p).is_ok() && p.is_dir() {
                candidates.push((p.display().to_string(), p.clone()));
            }
        }
    }
    deduplicate_roots(
        candidates
            .into_iter()
            .filter(|(_, path)| custodian_core::privacy::check(path).is_ok())
            .collect(),
    )
}

fn quarantine(app: &AppHandle) -> Result<Quarantine, String> {
    let dir = crate::paths::data_dir(app).map_err(|e| e.to_string())?;
    Ok(Quarantine::new(dir.join("quarantine")))
}

/// Adds `event` to the history ledger; returns a note for the reply if the
/// history needs the person's attention.
fn record_history(app: &AppHandle, event: serde_json::Value) -> Option<String> {
    let result = crate::paths::data_dir(app)
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
    let dir = crate::paths::data_dir(app).map_err(|e| e.to_string())?;
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
    if let Err(reason) =
        capability::Contract::for_call(&call).check(origin, state.cancel.load(Ordering::Acquire))
    {
        say(channel, reason);
        return Some(reason.into());
    }
    custodian_core::privacy::with_policy(state.privacy.clone(), || {
        if !custodian_core::privacy::request_is_current() {
            say(channel, "Privacy rules changed; this request was stopped.");
            return Some("Privacy rules changed; this request was stopped.".into());
        }
        let settings_mutation =
            capability::Contract::for_call(&call).effect == capability::Effect::ChangeSetting;
        let run = || {
            if let Err(reason) = capability::Contract::for_call(&call)
                .check(origin, state.cancel.load(Ordering::Acquire))
            {
                say(channel, reason);
                return Some(reason.into());
            }
            execute_inner(app, state, call, origin, channel)
        };
        if settings_mutation {
            custodian_core::privacy::with_mutation(run)
        } else {
            run()
        }
    })
}

fn execute_inner(
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
        let plan = match file_ops::prepare_call(app, state, &call) {
            Ok(plan) => plan,
            Err(err) => {
                say(channel, &err);
                return Some(format!(
                    "The request could not be prepared: {err}. No change was started."
                ));
            }
        };
        let _admission = state
            .request_admission
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if state.cancel.load(Ordering::Acquire) || !custodian_core::privacy::request_is_current() {
            return Some(STOPPED.into());
        }
        if let Some(card) = state
            .actions
            .add_prepared(call.clone(), Instant::now(), plan)
        {
            let note = actions::waiting_note(&call);
            emit(channel, ChatEvent::Confirm { card });
            return Some(note);
        }
    }
    match call {
        ToolCall::CheckDiskSpace => disk_space_tool(channel),
        ToolCall::FindLargeFiles { scope, count } => {
            let resolved = resolve_scope(app, &scope);
            let labels: Vec<&str> = resolved.iter().map(|(l, _)| l.as_str()).collect();
            let desc = format!("Measuring largest files in {}", reply::join_list(&labels));
            let (task_id, task_cancel) =
                match state.create_task(desc, custodian_core::coordinator::TaskState::Running, &[])
                {
                    Ok(task) => task,
                    Err(err) => {
                        say(
                            channel,
                            format!(
                                "I couldn't start tracking this scan: {err}. No scan was started."
                            ),
                        );
                        return None;
                    }
                };
            let search = state.file_references.begin_search();
            let files = run_largest(&resolved, count, &task_cancel, channel);
            state.file_references.complete_search(search, &files);
            let next =
                if task_cancel.load(Ordering::Relaxed) || state.cancel.load(Ordering::Relaxed) {
                    custodian_core::coordinator::TaskState::Cancelled
                } else {
                    custodian_core::coordinator::TaskState::Completed
                };
            state.coordinator.update_state(&task_id, next, None).ok();
        }
        ToolCall::FindDuplicateFiles(scope) => find_duplicates_tool(app, state, &scope, channel),
        ToolCall::FindFiles { name, scope } => {
            let resolved = resolve_scope(app, &scope);
            let labels: Vec<&str> = resolved.iter().map(|(l, _)| l.as_str()).collect();
            let desc = format!("Finding \"{}\" in {}", name, reply::join_list(&labels));
            let (task_id, task_cancel) =
                match state.create_task(desc, custodian_core::coordinator::TaskState::Running, &[])
                {
                    Ok(task) => task,
                    Err(err) => {
                        say(
                            channel,
                            format!(
                                "I couldn't start tracking this scan: {err}. No scan was started."
                            ),
                        );
                        return None;
                    }
                };
            let search = state.file_references.begin_search();
            let files = run_find(&resolved, &name, &task_cancel, channel);
            state.file_references.complete_search(search, &files);
            let next =
                if task_cancel.load(Ordering::Relaxed) || state.cancel.load(Ordering::Relaxed) {
                    custodian_core::coordinator::TaskState::Cancelled
                } else {
                    custodian_core::coordinator::TaskState::Completed
                };
            state.coordinator.update_state(&task_id, next, None).ok();
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
        ToolCall::UndoLastCleanup => undo_tool(app, state, channel),
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
        ToolCall::MoveFile {
            source,
            destination,
        } => move_file_tool(app, state, &source, &destination, channel),
        ToolCall::CopyFile {
            source,
            destination,
        } => copy_file_tool(app, state, &source, &destination, channel),
        ToolCall::RenameFile { path, new_name } => {
            rename_file_tool(app, state, &path, &new_name, channel)
        }
        ToolCall::CreateFolder(path) => create_folder_tool(app, state, &path, channel),
        ToolCall::GetFileInfo(path) => get_file_info_tool(app, state, &path, channel),
    }
    None
}

/// Where the sealed record of settings changes is kept.
fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    crate::paths::data_dir(app)
        .map(|dir| dir.join("settings.dpapi"))
        .map_err(|e| e.to_string())
}

/// Applies a planned settings change with pre-mutation verification, intent
/// persistence before mutation, and post-mutation read-back verification (F23 / F24).
fn apply_setting_change(
    app: &AppHandle,
    change: &custodian_winsec::settings::Change,
    channel: Sink<'_>,
) {
    let Some(setting) = change.setting() else {
        return say(channel, settings::unknown_text(&change.id));
    };

    // 1. Check if setting changed externally since plan was created
    let current_before = setting.current().map(|c| c.key.to_string());
    if current_before != change.from {
        return say(
            channel,
            format!(
                "The setting \"{}\" was changed outside Custodian (now \"{}\") since this was planned, so nothing was changed. Please ask again for a new plan.",
                setting.label,
                current_before.as_deref().unwrap_or("not set")
            ),
        );
    }

    // 2. Persist intent before touching the registry
    let path = match settings_path(app) {
        Ok(path) => path,
        Err(err) => {
            return say(
                channel,
                format!("I couldn't prepare my record of settings changes ({err}). Nothing was changed."),
            );
        }
    };
    let mut history = match settings::load(&path) {
        Ok(history) => history,
        Err(err) => {
            return say(
                channel,
                format!(
                    "I couldn't read my record of settings changes ({err}). Nothing was changed."
                ),
            );
        }
    };
    let now = now_unix() as i64;
    let entry = history.record(change.clone(), now);
    if let Err(err) = settings::save(&path, &mut history, now) {
        return say(
            channel,
            format!(
                "I couldn't record this change before applying it ({err}). Nothing was changed."
            ),
        );
    }

    // 3. Apply the setting
    if let Err(err) = setting.apply(&change.value) {
        // Rollback recorded history entry if apply failed
        history.mark_reverted(entry.id);
        let _ = settings::save(&path, &mut history, now);
        return say(channel, err.to_string());
    }

    // 4. Post-mutation read-back verification
    let read_back = setting.current().map(|c| c.key.to_string());
    if read_back.as_deref() != Some(&change.value) {
        // Read-back failed: revert change and record failure
        let _ = custodian_winsec::settings::revert(change);
        history.mark_reverted(entry.id);
        let _ = settings::save(&path, &mut history, now);
        return say(
            channel,
            format!(
                "I tried to change \"{}\" to \"{}\", but Windows reported \"{}\" on read-back. I put it back to avoid leaving an unverified state.",
                setting.label,
                change.value,
                read_back.as_deref().unwrap_or("not set")
            ),
        );
    }

    let note = record_history(
        app,
        json!({
            "kind": "setting_change",
            "setting": change.id,
            "value": change.value,
            "from": change.from,
            "at": now,
            "verified": true,
        }),
    );
    let text = settings::changed_text(change);
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

/// Puts back the most recent settings change from the sealed record, using
/// compare-and-swap semantics to ensure newer external user choices are not overwritten (F24).
fn revert_setting_tool(app: &AppHandle, channel: Sink<'_>) {
    let path = match settings_path(app) {
        Ok(path) => path,
        Err(err) => {
            return say(
                channel,
                format!("I couldn't open my record of settings changes: {err}"),
            );
        }
    };
    let mut history = match settings::load(&path) {
        Ok(history) => history,
        Err(err) => {
            return say(
                channel,
                format!("I couldn't read my record of settings changes: {err}"),
            );
        }
    };
    let Some(entry) = history.last_open().cloned() else {
        return say(channel, settings::NOTHING_TO_REVERT);
    };
    let Some(setting) = entry.change.setting() else {
        return say(channel, settings::unknown_text(&entry.change.id));
    };

    // Compare-and-swap: verify setting is still at the value Custodian set it to
    let current = setting.current().map(|c| c.key.to_string());
    if current.as_deref() != Some(&entry.change.value) {
        return say(
            channel,
            format!(
                "I couldn't put \"{}\" back because it was changed outside Custodian (currently \"{}\") since Custodian set it to \"{}\". Your newer choice was preserved.",
                setting.label,
                current.as_deref().unwrap_or("not set"),
                entry.change.value
            ),
        );
    }

    if let Err(err) = custodian_winsec::settings::revert(&entry.change) {
        return say(channel, format!("I couldn't put that back: {err}"));
    }

    history.mark_reverted(entry.id);
    let now = now_unix();
    let saved = settings::save(&path, &mut history, now as i64).err();
    let note = record_history(
        app,
        json!({"kind": "setting_revert", "setting": entry.change.id, "at": now, "verified": true}),
    );
    let mut text = settings::reverted_text(&entry.change);
    if let Some(err) = saved {
        text.push_str(&format!(
            "\n\nHeads up: I put it back, but couldn't update my own record ({err}), so I may offer to put this back again."
        ));
    }
    say(channel, with_note(text, note));
}

/// Returns true if `path` touches protected Windows system folders, caches or roots.
fn is_protected_system_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if [
        "pagefile.sys",
        "swapfile.sys",
        "hiberfil.sys",
        "bootmgr",
        "bootnxt",
        "bcd",
        "ntuser.dat",
        "usrclass.dat",
        "$mft",
        "$logfile",
        "$bitmap",
        "$secure",
    ]
    .contains(&name.as_str())
    {
        return true;
    }
    // Discovery exclusions are an optimisation, not a mutation allow-list.
    // Ordinary project/build folders on any drive are eligible for approved tasks.
    const PROTECTED_DIRS: &[&str] = &[
        "windows",
        "program files",
        "program files (x86)",
        "programdata",
        "appdata",
        "$recycle.bin",
        "system volume information",
        ".git",
        ".ssh",
    ];
    let lower_str = path.to_string_lossy().to_ascii_lowercase();
    for root in ["boot", "efi", "recovery", "$extend"] {
        if lower_str.contains(&format!(":\\{root}\\"))
            || lower_str.ends_with(&format!(":\\{root}"))
            || lower_str.contains(&format!(":/{root}/"))
            || lower_str.ends_with(&format!(":/{root}"))
        {
            return true;
        }
    }
    for dir in PROTECTED_DIRS {
        let needle1 = format!("\\{dir}\\");
        let needle2 = format!("/{dir}/");
        let prefix1 = format!(":\\{dir}\\");
        let prefix2 = format!(":/{dir}/");
        let end1 = format!("\\{dir}");
        let end2 = format!("/{dir}");
        if lower_str.contains(&needle1)
            || lower_str.contains(&needle2)
            || lower_str.contains(&prefix1)
            || lower_str.contains(&prefix2)
            || lower_str.ends_with(&end1)
            || lower_str.ends_with(&end2)
        {
            return true;
        }
    }
    for comp in path.components() {
        let s = comp.as_os_str().to_string_lossy().to_ascii_lowercase();
        if PROTECTED_DIRS.contains(&s.as_str()) {
            return true;
        }
    }
    path.parent().is_none()
}

fn resolve_existing_file(app: &AppHandle, raw: &str) -> Option<PathBuf> {
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return (custodian_core::privacy::check(&p).is_ok() && p.is_file()).then_some(p);
    }
    let mut matches = Vec::new();
    for folder in [
        KnownFolder::Documents,
        KnownFolder::Downloads,
        KnownFolder::Desktop,
        KnownFolder::Pictures,
        KnownFolder::Videos,
        KnownFolder::Music,
    ] {
        if let Some(root) = folder_path(app, folder) {
            let candidate = root.join(raw);
            if custodian_core::privacy::check(&candidate).is_ok() && candidate.is_file() {
                if let Ok(path) = candidate.canonicalize() {
                    if !matches.contains(&path) {
                        matches.push(path);
                    }
                }
            }
        }
    }
    (matches.len() == 1).then(|| matches.remove(0))
}

fn resolve_dest_dir(app: &AppHandle, raw: &str) -> Option<PathBuf> {
    if let Some(kf) = KnownFolder::from_label(raw) {
        if let Some(p) = folder_path(app, kf) {
            return custodian_core::privacy::check(&p).ok().map(|_| p);
        }
    }
    let p = PathBuf::from(raw);
    if custodian_core::privacy::check(&p).is_ok() && p.is_dir() {
        return Some(p);
    }
    None
}

fn move_file_tool(
    app: &AppHandle,
    state: &AssistantState,
    source: &str,
    destination: &str,
    channel: Sink<'_>,
) {
    file_ops::move_or_copy(
        app,
        state,
        custodian_core::file_tasks::FileAction::Move,
        source,
        destination,
        channel,
    );
}
fn copy_file_tool(
    app: &AppHandle,
    state: &AssistantState,
    source: &str,
    destination: &str,
    channel: Sink<'_>,
) {
    file_ops::move_or_copy(
        app,
        state,
        custodian_core::file_tasks::FileAction::Copy,
        source,
        destination,
        channel,
    );
}
fn rename_file_tool(
    app: &AppHandle,
    state: &AssistantState,
    path: &str,
    new_name: &str,
    channel: Sink<'_>,
) {
    file_ops::rename(app, state, path, new_name, channel);
}
fn create_folder_tool(app: &AppHandle, state: &AssistantState, path: &str, channel: Sink<'_>) {
    file_ops::create_folder(app, state, path, channel);
}

fn get_file_info_tool(app: &AppHandle, state: &AssistantState, path: &str, channel: Sink<'_>) {
    let raw = PathBuf::from(path);
    if raw.is_absolute() && custodian_core::privacy::check(&raw).is_err() {
        return say(channel, custodian_core::privacy::DENIED);
    }
    if raw.is_absolute() && !custodian_winsec::fsops::is_local_volume(&raw) {
        return say(
            channel,
            "Offline file inspection supports local drives only.",
        );
    }
    let target_path = if let Some(result) = state.file_references.resolve(path) {
        match result {
            Ok(path) => path,
            Err(err) => return say(channel, err),
        }
    } else if let Some(p) = resolve_existing_file(app, path) {
        p
    } else if let Some(d) = resolve_dest_dir(app, path) {
        d
    } else {
        let p = PathBuf::from(path);
        if custodian_core::privacy::check(&p).is_ok() && p.exists() {
            p
        } else {
            return say(
                channel,
                format!("I couldn't find \"{}\" on this PC.", reply::safe_name(path)),
            );
        }
    };

    let inspection = state.file_references.begin_search();
    if custodian_core::privacy::check(&target_path).is_err() {
        return say(channel, custodian_core::privacy::DENIED);
    }
    let meta = match std::fs::metadata(&target_path) {
        Ok(m) => m,
        Err(e) => {
            return say(
                channel,
                format!("I couldn't inspect \"{}\": {e}", reply::safe_name(path)),
            );
        }
    };

    let canonical = target_path
        .canonicalize()
        .unwrap_or_else(|_| target_path.clone());
    let is_dir = meta.is_dir();
    let name = target_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let modified_str = meta.modified().ok().and_then(|t| {
        let duration = t.duration_since(std::time::UNIX_EPOCH).ok()?;
        let dt = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)?;
        Some(
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        )
    });

    let created_str = meta.created().ok().and_then(|t| {
        let duration = t.duration_since(std::time::UNIX_EPOCH).ok()?;
        let dt = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)?;
        Some(
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        )
    });

    let mut details = if is_dir {
        let entry_count = permitted_entry_count(&target_path).ok();
        let mut d = format!(
            "Folder details for \"{}\":\n• Full path: {}",
            reply::safe_name(&name),
            canonical.display()
        );
        if let Some(count) = entry_count {
            d.push_str(&format!("\n• Permitted items: {}", reply::count(count)));
        }
        d
    } else {
        let size_bytes = meta.len();
        let human_size = reply::bytes(size_bytes);
        format!(
            "File details for \"{}\":\n• Full path: {}\n• Size: {} ({} bytes)",
            reply::safe_name(&name),
            canonical.display(),
            human_size,
            size_bytes
        )
    };

    if let Some(m) = &modified_str {
        details.push_str(&format!("\n• Last modified: {m}"));
    }
    if let Some(c) = &created_str {
        details.push_str(&format!("\n• Created: {c}"));
    }
    if meta.permissions().readonly() {
        details.push_str("\n• Attributes: Read-only");
    }

    if !is_dir {
        state.file_references.complete_search(
            inspection,
            &[custodian_core::walk::FileEntry {
                path: canonical.clone(),
                size: meta.len(),
                modified: meta.modified().ok(),
            }],
        );
    }
    state.record_context(format!(
        "Inspected {}: path={}, size={} bytes, modified={}",
        if is_dir { "folder" } else { "file" },
        canonical.display(),
        if is_dir { 0 } else { meta.len() },
        modified_str.as_deref().unwrap_or("unknown")
    ));

    say(channel, details);
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
        [one] if custodian_core::privacy::check(&one.path).is_err() => say(channel, custodian_core::privacy::DENIED),
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
    crate::paths::data_dir(app)
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
    if !app.state::<AssistantState>().privacy.allows_text(text) {
        return;
    }
    use tauri::Emitter;
    let _ = app.emit(NOTICE_EVENT, text);
}

/// Frees the space of copies whose 30 days have passed, in the background:
/// shortly after start-up and every few hours after that.
pub fn start_safe_place_keeper(app: AppHandle) {
    // Legacy quarantine purge has not yet been migrated to the shared policy.
    // Keep its receipts for manual recovery; never run it automatically.
    let _ = app;
}

fn legacy_safe_place_keeper(app: AppHandle) {
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
    requested_scope: &Scope,
    channel: Sink<'_>,
) -> String {
    let revision = state.privacy.revision();
    let call = match parse_requested_tool(name, args, requested_scope) {
        Ok(call) => call,
        Err(problem) => return format!("That tool call was refused: {problem}."),
    };
    let res = as_model_result(channel, |sink| {
        execute(app, state, call.clone(), Origin::Model, sink)
    });
    if state.privacy.revision() != revision || !state.privacy.allows_text(&res) {
        return "Privacy rules changed or access was denied. No protected result is available."
            .into();
    }
    state.record_context(format!(
        "[Model Tool: {}] -> {}",
        actions::action_name(&call),
        res
    ));
    res
}

/// The user's scope controls discovery; a model guess cannot silently narrow it.
fn parse_requested_tool(
    name: &str,
    args: &serde_json::Value,
    requested_scope: &Scope,
) -> Result<ToolCall, String> {
    let mut args = args.clone();
    if matches!(name, "find_large_files" | "find_files") {
        if let Some(object) = args.as_object_mut() {
            object.remove("folders");
        }
    }
    ToolCall::parse(name, &args).map(|call| bind_read_scope(call, requested_scope))
}

fn bind_read_scope(call: ToolCall, requested_scope: &Scope) -> ToolCall {
    match call {
        ToolCall::FindLargeFiles { count, .. } => ToolCall::FindLargeFiles {
            scope: requested_scope.clone(),
            count,
        },
        ToolCall::FindFiles { name, .. } => ToolCall::FindFiles {
            name,
            scope: requested_scope.clone(),
        },
        other => other,
    }
}

fn find_duplicates_tool(app: &AppHandle, state: &AssistantState, scope: &Scope, channel: Sink<'_>) {
    let folders = resolve_scope(app, scope);
    let labels: Vec<&str> = folders.iter().map(|(l, _)| l.as_str()).collect();
    let desc = format!("Looking for duplicates in {}", reply::join_list(&labels));
    let (task_id, task_cancel) =
        match state.create_task(desc, custodian_core::coordinator::TaskState::Running, &[]) {
            Ok(task) => task,
            Err(err) => {
                say(
                    channel,
                    format!("I couldn't start tracking this scan: {err}. No scan was started."),
                );
                return;
            }
        };
    if let Some(plan) = run_duplicates(&folders, &task_cancel, channel) {
        state
            .coordinator
            .update_state(
                &task_id,
                custodian_core::coordinator::TaskState::Completed,
                Some("Scan finished; the clean-up plan is awaiting approval.".into()),
            )
            .ok();
        state
            .plans
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(plan.id.clone(), PendingPlan::Duplicates(plan));
    } else {
        let next = if task_cancel.load(Ordering::Relaxed) || state.cancel.load(Ordering::Relaxed) {
            custodian_core::coordinator::TaskState::Cancelled
        } else {
            custodian_core::coordinator::TaskState::Completed
        };
        state.coordinator.update_state(&task_id, next, None).ok();
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
    folders: &[(String, PathBuf)],
    cancel: &AtomicBool,
    channel: Sink<'_>,
) -> Option<CleanupPlan> {
    if folders.is_empty() {
        say(channel, "I couldn't find those locations on this PC.");
        return None;
    }
    let labels: Vec<&str> = folders.iter().map(|(l, _)| l.as_str()).collect();
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

fn run_largest(
    folders: &[(String, PathBuf)],
    count: usize,
    cancel: &AtomicBool,
    channel: Sink<'_>,
) -> Vec<custodian_core::walk::FileEntry> {
    if folders.is_empty() {
        say(channel, "I couldn't find those locations on this PC.");
        return Vec::new();
    }
    let labels: Vec<&str> = folders.iter().map(|(l, _)| l.as_str()).collect();
    emit(
        channel,
        ChatEvent::Status {
            text: format!("Measuring files in {}…", reply::join_list(&labels)),
        },
    );
    let roots: Vec<PathBuf> = folders.iter().map(|(_, p)| p.clone()).collect();
    let mut throttle = Throttle(Instant::now());
    let limits = WalkLimits::whole_system_read();
    let (files, stats) = largest_files(&roots, count, limits, cancel, &mut |n| {
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
        return Vec::new();
    }
    say(channel, reply::largest_summary(&files, &labels, &stats));
    files
}

/// "Where's my resume?": files whose names contain the words, newest first.
fn run_find(
    folders: &[(String, PathBuf)],
    name: &str,
    cancel: &AtomicBool,
    channel: Sink<'_>,
) -> Vec<custodian_core::walk::FileEntry> {
    if folders.is_empty() {
        say(channel, "I couldn't find those locations on this PC.");
        return Vec::new();
    }
    let labels: Vec<&str> = folders.iter().map(|(l, _)| l.as_str()).collect();
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
        WalkLimits::whole_system_read(),
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
        return Vec::new();
    }
    say(
        channel,
        reply::find_summary(name, &files, &labels, stats, SystemTime::now()),
    );
    files
}

/// The PC's fixed drives: (name, free bytes, total bytes).
fn fixed_drives() -> Vec<(String, u64, u64)> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut drives: Vec<(String, u64, u64)> = disks
        .list()
        .iter()
        .filter(|d| {
            d.total_space() > 0 && custodian_winsec::fsops::is_local_volume(d.mount_point())
        })
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

#[cfg(windows)]
fn catalogued_app_path(app_kind: App) -> Result<PathBuf, String> {
    use custodian_winsec::registry::{string_value, Hive};
    let root = string_value(
        Hive::LocalMachine,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "SystemRoot",
    )
    .ok_or("Windows installation location could not be verified")?;
    let root = PathBuf::from(root);
    if !root.is_absolute() {
        return Err("Windows installation location is not absolute".into());
    }
    let path = match app_kind {
        App::Notepad => root.join("System32/notepad.exe"),
        App::Calculator => root.join("System32/calc.exe"),
        App::Paint => root.join("System32/mspaint.exe"),
        App::FileExplorer => root.join("explorer.exe"),
        App::TaskManager => root.join("System32/taskmgr.exe"),
        App::Settings => return Err("settings".into()),
    };
    if !path.is_file() {
        return Err("The catalogued Windows application is unavailable".into());
    }
    Ok(path)
}

fn permitted_entry_count(path: &Path) -> std::io::Result<usize> {
    custodian_core::privacy::check(path)?;
    std::fs::read_dir(path)?.try_fold(0_usize, |count, entry| {
        let entry = entry?;
        if !custodian_core::privacy::request_is_current() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Request stopped",
            ));
        }
        Ok(count + usize::from(custodian_core::privacy::check(&entry.path()).is_ok()))
    })
}

#[cfg(windows)]
fn catalogued_app_command(app_kind: App) -> Result<std::process::Command, String> {
    let (exe, package) = match catalogued_app_path(app_kind) {
        Ok(exe) => (exe, None),
        // Current Windows may provide Paint only as a packaged application.
        // Activate its fixed Microsoft package ID through trusted Explorer,
        // never through a user-writable executable alias or PATH lookup.
        Err(_) if app_kind == App::Paint => (
            catalogued_app_path(App::FileExplorer)?,
            Some(r"shell:AppsFolder\Microsoft.Paint_8wekyb3d8bbwe!App"),
        ),
        Err(error) => return Err(error),
    };
    let mut command = std::process::Command::new(&exe);
    command.current_dir(exe.parent().ok_or("Missing application folder")?);
    if let Some(package) = package {
        command.arg(package);
    }
    Ok(command)
}

fn open_app(app_kind: App) -> Result<(), String> {
    #[cfg(windows)]
    {
        catalogued_app_command(app_kind)?
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
        Some(path) if custodian_core::privacy::check(&path).is_err() => {
            say(channel, custodian_core::privacy::DENIED)
        }
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

fn undo_tool(app: &AppHandle, state: &AssistantState, channel: Sink<'_>) {
    if file_ops::undo_latest_if_newer(app, state, channel) {
        return;
    }
    say(channel, privacy::PAUSED);
}

fn legacy_undo_tool(app: &AppHandle, channel: Sink<'_>) {
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
    let runtime = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("runtime/agent/server.js"));
    [runtime, bundled, Some(dev)]
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
    let runtime = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("runtime/models"));
    [runtime, bundled, Some(dev)]
        .into_iter()
        .flatten()
        .find(|p| p.is_dir())
}

/// Starts the agent unless it is already running. Returns whether it had
/// to be started, or why it could not be.
fn ensure_agent(
    app: &AppHandle,
    agent: &mut Option<AgentProcess>,
    cancel: Option<&AtomicBool>,
) -> Result<bool, String> {
    if agent.as_mut().is_some_and(|a| !a.is_alive()) {
        *agent = None;
    }
    if agent.is_some() {
        return Ok(false);
    }
    let entry = agent_entry(app)
        .ok_or("the agent isn't built yet (run `npm run build` in the repository)")?;
    let node = std::env::var("CUSTODIAN_NODE").unwrap_or_else(|_| {
        app.path()
            .resource_dir()
            .ok()
            .map(|dir| dir.join("runtime/bin/node.exe"))
            .filter(|path| path.is_file())
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| "node".into())
    });
    let models = models_dir(app);
    #[cfg(windows)]
    {
        let opt_out = std::env::var_os("CUSTODIAN_AGENT_SANDBOX").is_some_and(|v| v == "0")
            || std::env::var_os("CUSTODIAN_AGENT_UNRESTRICTED").is_some_and(|v| v == "1");
        if opt_out {
            eprintln!("[desktop] WARNING: Agent sandbox is explicitly disabled via CUSTODIAN_AGENT_SANDBOX=0 / CUSTODIAN_AGENT_UNRESTRICTED=1 (development mode only)");
            let started =
                AgentProcess::spawn(&node, &entry, models.as_deref()).map_err(|e| e.to_string())?;
            *LAST_LAUNCH_SANDBOXED
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(started.sandboxed());
            *agent = Some(started);
            return Ok(true);
        }

        match spawn_sandboxed_agent(&node, &entry, models.as_deref(), cancel) {
            Ok(sandboxed) => {
                *LAST_LAUNCH_SANDBOXED
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(sandboxed.sandboxed());
                *agent = Some(sandboxed);
                Ok(true)
            }
            Err(err) => {
                *LAST_LAUNCH_SANDBOXED
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(false);
                Err(format!(
                    "The AI agent could not be started securely in its sandbox: {err}. Unrestricted child startup is refused. Deterministic tools remain available."
                ))
            }
        }
    }
    #[cfg(not(windows))]
    {
        let started =
            AgentProcess::spawn(&node, &entry, models.as_deref()).map_err(|e| e.to_string())?;
        *LAST_LAUNCH_SANDBOXED
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(started.sandboxed());
        *agent = Some(started);
        Ok(true)
    }
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
    let history_intact = crate::paths::data_dir(app)
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
/// files) and checks that it can load its model there.
#[cfg(windows)]
fn spawn_sandboxed_agent(
    node: &str,
    entry: &Path,
    models: Option<&Path>,
    cancel: Option<&AtomicBool>,
) -> Result<AgentProcess, String> {
    let attempt = || -> Result<AgentProcess, String> {
        let node = crate::agent::resolve_node(node).ok_or("node was not found on the PATH")?;
        let sandbox = agent_sandbox(&node, entry, models)?;
        let mut agent = AgentProcess::spawn_sandboxed(&sandbox, &node, entry, models)
            .map_err(|e| e.to_string())?;
        match agent.ping_with_cancel(cancel).map_err(|e| e.to_string())? {
            None => Ok(agent),
            Some(problem) => Err(problem),
        }
    };
    match attempt() {
        Ok(agent) => {
            eprintln!(
                "[desktop] the agent runs in its sandbox (no network, no access to your files)"
            );
            Ok(agent)
        }
        Err(reason) => {
            eprintln!("[desktop] the agent could not run in its sandbox: {reason}");
            *SANDBOX_PROBLEM.lock().unwrap_or_else(|e| e.into_inner()) = Some(reason.clone());
            Err(reason)
        }
    }
}

/// Starts the agent ahead of time (when the chat panel opens), so the model
/// is loaded by the time the first question arrives. Failures are reported
/// later, on that question.
pub fn prewarm(app: &AppHandle) {
    let state = app.state::<AssistantState>();
    let mut agent = state.agent.lock().unwrap_or_else(|e| e.into_inner());
    if ensure_agent(app, &mut agent, Some(&state.cancel)).is_ok() {
        mark_agent_used(&state);
    }
}

/// Answers open-ended messages with the local model.
fn ask_model(app: &AppHandle, state: &AssistantState, message: &str, channel: Sink<'_>) {
    use custodian_core::coordinator::TaskState;
    let (task_id, cancel) =
        match state.create_task("Answering a chat request".into(), TaskState::Queued, &[]) {
            Ok(task) => task,
            Err(err) => return say(channel, err.to_string()),
        };
    let mut agent = state.agent.lock().unwrap_or_else(|e| e.into_inner());
    let revision = state.privacy.revision();
    let app_revision = match state.app_privacy.settings() {
        Ok(settings) => settings.revision,
        Err(_) => return say(channel, "Application privacy rules could not be verified."),
    };
    if state.agent_policy_revision.load(Ordering::Acquire) != revision
        || state.agent_app_policy_revision.load(Ordering::Acquire) != app_revision
    {
        *agent = None;
        state
            .agent_policy_revision
            .store(revision, Ordering::Release);
        state
            .agent_app_policy_revision
            .store(app_revision, Ordering::Release);
    }
    if cancel.load(Ordering::Relaxed) || !custodian_core::privacy::request_is_current() {
        state
            .coordinator
            .update_state(&task_id, TaskState::Cancelled, None)
            .ok();
        return say(channel, STOPPED);
    }
    if let Err(err) = state
        .coordinator
        .update_state(&task_id, TaskState::Running, None)
    {
        return say(channel, err.to_string());
    }
    match ensure_agent(app, &mut agent, Some(&cancel)) {
        Ok(true) => emit(
            channel,
            ChatEvent::Status {
                text: "Waking up my brain… the first answer takes a moment.".into(),
            },
        ),
        Ok(false) => {}
        Err(err) => {
            if cancel.load(Ordering::Relaxed) {
                state
                    .coordinator
                    .update_state(&task_id, TaskState::Cancelled, None)
                    .ok();
                return say(channel, STOPPED);
            }
            state
                .coordinator
                .update_state(&task_id, TaskState::Failed, Some(err.clone()))
                .ok();
            say(channel, reply::no_brain(&err));
            return;
        }
    }
    let Some(process) = agent.as_mut() else {
        state
            .coordinator
            .update_state(
                &task_id,
                TaskState::Failed,
                Some("The agent was not started.".into()),
            )
            .ok();
        return;
    };
    emit(
        channel,
        ChatEvent::Status {
            text: "Thinking…".into(),
        },
    );
    let context_prefix = state.get_context_projection();
    let prompt = if context_prefix.is_empty() {
        message.to_string()
    } else {
        format!("{context_prefix}{message}")
    };
    let result = process.chat(
        &prompt,
        &cancel,
        &mut |text| say(channel, text),
        &mut |name, args| {
            if cancel.load(Ordering::Relaxed) || state.privacy.revision() != revision {
                return "This request was stopped or its permissions changed. No further action was started.".into();
            }
            run_model_tool(
                app,
                state,
                name,
                args,
                &intent::read_scope_for(message),
                channel,
            )
        },
    );
    mark_agent_used(state);
    let next = if cancel.load(Ordering::Relaxed) {
        TaskState::Cancelled
    } else if result.is_ok() {
        TaskState::Completed
    } else {
        TaskState::Failed
    };
    state.coordinator.update_state(&task_id, next, None).ok();
    if cancel.load(Ordering::Relaxed) {
        if result.is_err() {
            *agent = None;
        }
        return say(channel, STOPPED);
    }
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
fn respond(
    app: &AppHandle,
    state: &AssistantState,
    message: &str,
    channel: Sink<'_>,
    routine_revision: u64,
) {
    if state.cancel.load(Ordering::Acquire) || !custodian_core::privacy::request_is_current() {
        return say(channel, STOPPED);
    }
    if !state.privacy.allows_text(message) {
        return say(channel, custodian_core::privacy::DENIED);
    }
    if matches!(
        message.trim().trim_end_matches('?').to_lowercase().as_str(),
        "what can you do" | "what are your capabilities" | "help"
    ) {
        return say(channel, privacy::capability_text());
    }
    if routine::handle(app, state, message, channel, routine_revision) {
        return;
    }
    if app_tasks::handle(app, state, message, channel) {
        return;
    }
    if workflow::handle(app, state, message, channel, routine_revision) {
        return;
    }
    let parsed = intent::parse(message);
    match parsed {
        Intent::CloseCubby => {
            say(
                channel,
                "I can't close myself, but you can hide me from the tray icon, or say \"go to sleep\".",
            );
            state.record_context(
                "User requested to close Cubby: informed how to hide or sleep.".into(),
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
            state.record_context("User requested sleep: Cubby entered sleep state.".into());
            return;
        }
        _ => {}
    }

    // A settings request is resolved against the catalogue before it can
    // become a call, so nothing outside that catalogue is ever changed.
    if let Intent::Settings(words) = &parsed {
        let captured = RefCell::new(Vec::new());
        let sink = |event: ChatEvent| {
            if let ChatEvent::Text { ref text } = event {
                captured.borrow_mut().push(text.clone());
            }
            channel(event);
        };
        if resolve_settings_request(app, state, words, &sink) {
            let output = captured.into_inner().join("\n\n");
            if !output.is_empty() {
                state.record_context(format!(
                    "[Settings Request] User asked: \"{}\" -> Result: {}",
                    message, output
                ));
            }
            return;
        }
    }

    if let Some(call) = ToolCall::from_intent(&parsed) {
        let captured = RefCell::new(Vec::new());
        let sink = |event: ChatEvent| {
            if let ChatEvent::Text { ref text } = event {
                captured.borrow_mut().push(text.clone());
            }
            channel(event);
        };
        execute(app, state, call.clone(), Origin::Person, &sink);
        let output = captured.into_inner().join("\n\n");
        if !output.is_empty() {
            state.record_context(format!(
                "[Direct Action: {}] User asked: \"{}\" -> Result: {}",
                actions::action_name(&call),
                message,
                output
            ));
        }
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
    // Capture authority before queueing a worker, not after a grant changes
    // while the request waits for the blocking pool.
    let epoch = app.state::<AssistantState>().request_epoch();
    let routine_revision = app.state::<AssistantState>().routine.revision();
    let revision = app.state::<AssistantState>().privacy.revision();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        if !state.begin_request(epoch, revision) {
            let _ = channel.send(ChatEvent::Text { text: "This queued request was stopped or its privacy rules changed. Ask again to start a new request.".into() });
            return;
        }
        // A closed chat panel is not an error.
        let sink = |event: ChatEvent| {
            state.privacy.with_revision(revision, || {
                let _ = channel.send(event);
            });
        };
        custodian_core::request::with_epoch(state.request_epoch.clone(), epoch, || {
          custodian_core::privacy::with_policy_revision(state.privacy.clone(), revision, || {
            respond(&app, &state, &message, &sink, routine_revision)
          })
        });
    })
    .await
    .map_err(|e| e.to_string())
}

/// The Stop button: ends all scans, tasks or answers in progress.
#[tauri::command]
pub fn cancel_chat(state: tauri::State<'_, AssistantState>) {
    state.stop_requests();
    state.actions.clear();
}

/// Lists currently active background tasks from the coordinator.
#[tauri::command]
pub fn list_active_tasks(
    state: tauri::State<'_, AssistantState>,
) -> Vec<custodian_core::coordinator::TaskInfo> {
    let mut visible = state
        .coordinator
        .list_all_tasks()
        .into_iter()
        .filter(|task| {
            task.state.is_active()
                || task.state == custodian_core::coordinator::TaskState::RecoveryRequired
                || task.execution.is_some()
        })
        .filter(|task| task_is_visible(&state.privacy, task))
        .filter(|task| {
            task.application.as_ref().is_none_or(|binding| {
                binding.identity.as_ref().map_or_else(
                    || {
                        state
                            .app_privacy
                            .admit_document_source(&binding.executable, &binding.document_id)
                            .is_ok()
                    },
                    |identity| {
                        state
                            .app_privacy
                            .task_visible(identity, &binding.document_id)
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    visible.sort_by_key(|task| {
        (
            std::cmp::Reverse(task.state.is_active()),
            std::cmp::Reverse(task.updated_at),
            task.id.clone(),
        )
    });
    let mut history = 0;
    visible.retain(|task| {
        if task.state.is_active()
            || task.state == custodian_core::coordinator::TaskState::RecoveryRequired
        {
            true
        } else {
            history += 1;
            history <= 10
        }
    });
    visible
}

fn task_is_visible(
    policy: &custodian_core::privacy::AccessPolicy,
    task: &custodian_core::coordinator::TaskInfo,
) -> bool {
    policy.allows_text(&task.description)
        && task
            .progress
            .as_ref()
            .is_none_or(|progress| policy.allows_text(progress))
        && task
            .resources
            .iter()
            .all(|resource| policy.allows(Path::new(resource)))
        && task.execution.as_ref().is_none_or(|execution| {
            serde_json::to_string(execution).is_ok_and(|text| policy.allows_text(&text))
        })
}

/// Cancels an individual task by its ID.
#[tauri::command]
pub fn cancel_task(state: tauri::State<'_, AssistantState>, task_id: String) -> bool {
    state.coordinator.cancel_task(&task_id)
}

#[tauri::command]
pub fn pause_task(
    state: tauri::State<'_, AssistantState>,
    task_id: String,
    paused: bool,
) -> Result<(), String> {
    state
        .coordinator
        .pause_program(&task_id, paused)
        .map_err(|e| e.to_string())
}

/// The person's answer to a card. Only the card's id comes from the
/// webview; what runs is the action Cubby stored when it made the card.
#[tauri::command]
pub async fn decide_action(
    app: AppHandle,
    action_id: String,
    allow: bool,
) -> Result<String, String> {
    let epoch = app.state::<AssistantState>().request_epoch();
    let revision = app.state::<AssistantState>().privacy.revision();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        if !state.begin_request(epoch, revision) {
            return Err(
                "This approval was stopped or its privacy rules changed. Ask again.".into(),
            );
        }
        if action_id.starts_with("app-") {
            return custodian_core::request::with_epoch(state.request_epoch.clone(), epoch, || {
                custodian_core::privacy::with_policy_revision(
                    state.privacy.clone(),
                    revision,
                    || app_tasks::decide(&app, &state, &action_id, allow),
                )
            });
        }
        let (call, file_plan) =
            custodian_core::privacy::with_policy_revision(state.privacy.clone(), revision, || {
                state.actions.take_prepared(&action_id, Instant::now())
            })
            .ok_or_else(|| CARD_GONE.to_string())?;
        if !allow {
            let text = actions::declined_text(&call);
            state.record_context(format!(
                "[Card Decision] Action \"{}\" was declined by user.",
                actions::action_name(&call)
            ));
            return Ok(text);
        }
        let said = RefCell::new(Vec::new());
        let sink = |event: ChatEvent| {
            if let ChatEvent::Text { text } = event {
                said.borrow_mut().push(text);
            }
        };
        custodian_core::request::with_epoch(state.request_epoch.clone(), epoch, || {
            custodian_core::privacy::with_policy_revision(state.privacy.clone(), revision, || {
                if let Err(reason) = capability::Contract::for_call(&call)
                    .check(Origin::Approved, state.cancel.load(Ordering::Acquire))
                {
                    say(&sink, reason);
                    return;
                }
                if let Some(plan) = file_plan {
                    file_ops::perform_prepared(&app, &state, &plan, &sink);
                } else {
                    execute(&app, &state, call.clone(), Origin::Approved, &sink);
                }
            });
        });
        let result = said.into_inner().join("\n\n");
        if state.privacy.revision() != revision
            || state.request_epoch() != epoch
            || !state.privacy.allows_text(&result)
        {
            return Err(
                "Privacy rules changed. Review the current task state before continuing.".into(),
            );
        }
        state.record_context(format!(
            "[Card Decision] Action \"{}\" was approved by user. Result: {}",
            actions::action_name(&call),
            result
        ));
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Executes a clean-up plan that Cubby proposed, after the person confirms it
/// with Windows Hello (or a native dialog where Hello is not set up). Only plans created by the assistant itself can be
/// approved; the webview never supplies paths and cannot approve on its own.
#[tauri::command]
pub async fn approve_plan(app: AppHandle, plan_id: String) -> Result<String, String> {
    let _ = (app, plan_id);
    Err(privacy::PAUSED.into())
}

async fn legacy_approve_plan(app: AppHandle, plan_id: String) -> Result<String, String> {
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
        let (summary, history_entry) = match &outcome {
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
                (
                    with_note(reply::cleanup_summary(outcome), note),
                    format!(
                        "Cleanup plan {} completed: moved {} files ({} bytes), skipped {}",
                        outcome.plan_id,
                        outcome.moved,
                        outcome.bytes,
                        outcome.skipped.len()
                    ),
                )
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
                (
                    with_note(reply::temp_summary(outcome), note),
                    format!(
                        "Temp cleanup plan completed: deleted {} files ({} bytes), skipped {}",
                        outcome.deleted, outcome.bytes, outcome.skipped
                    ),
                )
            }
        };
        state.record_context(format!("[Plan Decision] {history_entry}"));
        Ok(summary)
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
    state.record_context(format!(
        "[Plan Decision] Plan {plan_id} was rejected by user."
    ));
}

/// Resets the conversation context and agent session for a fresh conversation (R16).
#[tauri::command]
pub async fn reset_conversation(app: AppHandle) -> Result<(), String> {
    // Invalidate queued/old workers before waiting for the model mutex.
    let state = app.state::<AssistantState>();
    state.invalidate_conversation();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        let mut agent = state.agent.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(process) = agent.as_mut() {
            process.reset().map_err(|err| err.to_string())?;
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Past clean-ups for the history view.
#[tauri::command]
pub fn get_ledger(app: AppHandle) -> Result<Vec<String>, String> {
    let _ = app;
    Err(privacy::PAUSED.into())
}

fn legacy_get_ledger(app: AppHandle) -> Result<Vec<String>, String> {
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
    fn folder_counts_exclude_private_children_and_credentials() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("public.txt"), b"public").unwrap();
        fs::write(root.path().join("private.txt"), b"private").unwrap();
        fs::write(root.path().join(".env"), b"credentials").unwrap();
        let policy = Arc::new(custodian_core::privacy::AccessPolicy::default());
        policy
            .replace_private_paths(vec![root.path().join("private.txt")])
            .unwrap();
        custodian_core::privacy::with_policy(policy, || {
            assert_eq!(permitted_entry_count(root.path()).unwrap(), 1)
        });
    }

    #[cfg(windows)]
    #[test]
    fn catalogued_launches_resolve_absolute_protected_windows_paths() {
        for app in [
            App::Notepad,
            App::Calculator,
            App::Paint,
            App::FileExplorer,
            App::TaskManager,
        ] {
            let command = catalogued_app_command(app).unwrap();
            let path = PathBuf::from(command.get_program());
            assert!(path.is_absolute());
            assert!(is_protected_system_path(&path));
            assert_ne!(path.parent().unwrap(), std::env::current_dir().unwrap());
        }
    }

    #[test]
    fn conversation_reset_rejects_late_context_and_queued_work() {
        let state = AssistantState::default();
        let captured = state.request_epoch();
        state.record_context("old fact".into());
        custodian_core::request::with_epoch(state.request_epoch.clone(), captured, || {
            state.invalidate_conversation();
            assert!(!state.begin_request(captured, state.privacy.revision()));
            assert!(state.begin_request(state.request_epoch(), state.privacy.revision()));
            state.record_context("late stale result".into());
        });
        assert!(!state.get_context_projection().contains("old fact"));
        assert!(!state.get_context_projection().contains("late stale result"));
    }
    #[test]
    fn stop_and_privacy_changes_invalidate_queued_requests_before_they_can_clear_stop() {
        let state = AssistantState::default();
        let revision = state.privacy.revision();
        let queued = state.request_epoch();
        assert!(state.begin_request(queued, revision));
        state.stop_requests();
        assert!(!state.begin_request(queued, revision));
        assert!(state.cancel.load(Ordering::Acquire));
        let next = state.request_epoch();
        state.privacy.replace_private_paths(vec![]).unwrap();
        assert!(!state.begin_request(next, revision));
        assert!(state.cancel.load(Ordering::Acquire));
        assert!(state.begin_request(next, state.privacy.revision()));
        assert!(!state.cancel.load(Ordering::Acquire));
    }

    #[test]
    fn new_chat_does_not_revive_stopped_task_or_expose_newly_private_task_resources() {
        let state = AssistantState::default();
        let root = tempfile::tempdir().unwrap();
        let private = root.path().join("private");
        fs::create_dir(&private).unwrap();
        let epoch = state.request_epoch();
        custodian_core::request::with_epoch(state.request_epoch.clone(), epoch, || {
            let (id, cancel) = state
                .create_task(
                    "Generic progress".into(),
                    custodian_core::coordinator::TaskState::Running,
                    std::slice::from_ref(&private),
                )
                .unwrap();
            let task = state
                .coordinator
                .list_active_tasks()
                .into_iter()
                .find(|t| t.id == id)
                .unwrap();
            assert!(task_is_visible(&state.privacy, &task));
            state.privacy.replace_private_paths(vec![private]).unwrap();
            assert!(!task_is_visible(&state.privacy, &task));
            state.stop_requests();
            assert!(cancel.load(Ordering::Acquire));
            assert!(state.begin_request(state.request_epoch(), state.privacy.revision()));
            assert!(state
                .create_task(
                    "late child".into(),
                    custodian_core::coordinator::TaskState::Running,
                    &[]
                )
                .is_err());
        });
        custodian_core::request::with_epoch(
            state.request_epoch.clone(),
            state.request_epoch(),
            || {
                assert!(state
                    .create_task(
                        "independent answer".into(),
                        custodian_core::coordinator::TaskState::Running,
                        &[]
                    )
                    .is_ok());
            },
        );
    }

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
            ("Pictures".to_string(), pictures.clone()),
            ("Downloads".to_string(), downloads.clone()),
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
            &[("Videos".to_string(), dir.path().to_path_buf())],
            10,
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

    #[cfg(windows)]
    #[test]
    fn file_journey_finds_approves_moves_restarts_and_undoes() {
        use custodian_core::coordinator::{TaskCoordinator, TaskState};
        use custodian_core::file_tasks::{FileAction, FileTaskPlan, FileTasks, ReceiptState};
        let dir = tempfile::tempdir().unwrap();
        let originals = dir.path().join("originals");
        let target = dir.path().join("target");
        fs::create_dir(&originals).unwrap();
        fs::create_dir(&target).unwrap();
        let source = originals.join("largest.txt");
        let destination = target.join("largest.txt");
        fs::write(&source, vec![b'x'; 8192]).unwrap();
        fs::write(originals.join("small.txt"), b"small").unwrap();
        let events = RefCell::new(Vec::new());
        let sink = |e: ChatEvent| events.borrow_mut().push(e);
        let files = run_largest(
            &[("Fixture".into(), originals)],
            1,
            &AtomicBool::new(false),
            &sink,
        );
        let refs = references::FileReferences::default();
        refs.replace(&files);
        let selected = refs.resolve("it").unwrap().unwrap();
        assert_eq!(selected, source.canonicalize().unwrap());
        let plan = FileTaskPlan::prepare(FileAction::Move, Some(&selected), &destination).unwrap();
        let actions = PendingActions::default();
        let call = ToolCall::MoveFile {
            source: "it".into(),
            destination: target.display().to_string(),
        };
        let now = Instant::now();
        let card = actions.add_prepared(call, now, Some(plan)).unwrap();
        assert!(card.detail.contains("largest.txt"));
        assert!(source.exists(), "a card is not execution");
        assert!(!destination.exists());
        let (_, plan) = actions.take_prepared(&card.id, now).unwrap();
        assert!(
            actions.take_prepared(&card.id, now).is_none(),
            "approval is single-use"
        );
        let task_path = dir.path().join("tasks.dpapi");
        let receipts = dir.path().join("receipts");
        let tasks = TaskCoordinator::open(task_path.clone()).unwrap();
        let (task, _) = tasks
            .create_task(
                "Move selected file".into(),
                TaskState::Running,
                &[selected, destination.clone()],
            )
            .unwrap();
        let receipt = FileTasks::new(receipts.clone())
            .execute_plan(&plan.unwrap())
            .unwrap();
        tasks
            .update_state(&task, TaskState::Completed, None)
            .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), vec![b'x'; 8192]);
        assert!(!source.exists());
        drop(tasks);
        let restarted = TaskCoordinator::open(task_path).unwrap();
        assert_eq!(
            restarted.get_task(&task).unwrap().state,
            TaskState::Completed
        );
        let reopened = FileTasks::new(receipts);
        assert_eq!(reopened.receipts().unwrap()[0].id, receipt.id);
        reopened.undo(&receipt.id).unwrap();
        assert_eq!(fs::read(&source).unwrap(), vec![b'x'; 8192]);
        assert!(!destination.exists());
        assert_eq!(reopened.receipts().unwrap()[0].state, ReceiptState::Undone);
        assert!(reopened.undo(&receipt.id).is_err());
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
            &[("Documents".to_string(), docs.clone())],
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
            &[("Pictures".to_string(), dir.path().to_path_buf())],
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

    #[test]
    fn deduplicate_roots_retains_parent_when_child_is_listed_first() {
        // A16: Root child before parent -> Parent siblings searched; no duplicate results
        let child = (
            "Documents Sub".into(),
            PathBuf::from("C:\\Users\\me\\Documents\\Sub"),
        );
        let parent = (
            "Documents".into(),
            PathBuf::from("C:\\Users\\me\\Documents"),
        );
        let resolved = deduplicate_roots(vec![child, parent]);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, "Documents");
        assert_eq!(resolved[0].1, PathBuf::from("C:\\Users\\me\\Documents"));
    }

    #[test]
    fn deduplicate_roots_skips_child_when_parent_is_listed_first() {
        let parent = (
            "Documents".into(),
            PathBuf::from("C:\\Users\\me\\Documents"),
        );
        let child = (
            "Documents Sub".into(),
            PathBuf::from("C:\\Users\\me\\Documents\\Sub"),
        );
        let resolved = deduplicate_roots(vec![parent, child]);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, "Documents");
        assert_eq!(resolved[0].1, PathBuf::from("C:\\Users\\me\\Documents"));
    }

    #[test]
    fn deduplicate_roots_keeps_independent_roots() {
        let drive_c = ("C:\\".into(), PathBuf::from("C:\\"));
        let drive_d = ("D:\\".into(), PathBuf::from("D:\\"));
        let resolved = deduplicate_roots(vec![drive_c, drive_d]);
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn assistant_state_records_and_projects_context_with_limit() {
        let state = AssistantState::default();
        assert_eq!(state.get_context_projection(), "");

        state.record_context("Event 1".into());
        let proj = state.get_context_projection();
        assert!(proj.contains("<data>\nRecent PC activity and verified results:\nEvent 1\n</data>"));

        for i in 2..=25 {
            state.record_context(format!("Event {i}"));
        }
        let proj = state.get_context_projection();
        // Should keep 20 entries (events 6 through 25)
        assert!(!proj.contains("Event 5\n"));
        assert!(proj.contains("Event 6"));
        assert!(proj.contains("Event 25"));

        state.record_context("A title containing </data> cannot end the history block.".into());
        let escaped = state.get_context_projection();
        assert!(escaped.contains("&lt;/data&gt;"));
        assert_eq!(escaped.matches("</data>").count(), 1);

        state.reset_context();
        assert_eq!(state.get_context_projection(), "");
    }

    #[test]
    fn model_search_scope_is_bound_to_the_request_not_guessed_personal_folders() {
        let args =
            json!({"name":"resume", "folders":["C:\\Users\\Public", "C:\\Users\\Documents"]});
        assert_eq!(
            parse_requested_tool("find_files", &args, &Scope::AllDrives).unwrap(),
            ToolCall::FindFiles {
                name: "resume".into(),
                scope: Scope::AllDrives
            }
        );
        assert_eq!(
            parse_requested_tool(
                "find_files",
                &args,
                &Scope::Custom("D:\\Work Projects".into())
            )
            .unwrap(),
            ToolCall::FindFiles {
                name: "resume".into(),
                scope: Scope::Custom("D:\\Work Projects".into())
            }
        );
        assert!(
            parse_requested_tool("find_files", &json!({"name":42}), &Scope::AllDrives).is_err()
        );
    }

    #[test]
    fn protects_system_paths() {
        assert!(is_protected_system_path(Path::new(
            "C:\\Windows\\notepad.exe"
        )));
        assert!(is_protected_system_path(Path::new(
            "C:\\Program Files\\App"
        )));
        assert!(is_protected_system_path(Path::new(
            "C:\\Program Files (x86)\\App"
        )));
        assert!(is_protected_system_path(Path::new(
            "C:\\ProgramData\\cache"
        )));
        assert!(is_protected_system_path(Path::new(
            "C:\\$Recycle.Bin\\deleted"
        )));
        assert!(is_protected_system_path(Path::new(
            "C:\\System Volume Information"
        )));
        assert!(is_protected_system_path(Path::new("C:\\")));

        assert!(!is_protected_system_path(Path::new(
            "C:\\Users\\User\\Documents\\file.txt"
        )));
        assert!(!is_protected_system_path(Path::new(
            "D:\\Projects\\repo\\test.rs"
        )));
        assert!(!is_protected_system_path(Path::new(
            "D:\\Projects\\target\\report.txt"
        )));
        assert!(is_protected_system_path(Path::new("C:\\pagefile.sys")));
        assert!(is_protected_system_path(Path::new(
            "C:\\Users\\User\\NTUSER.DAT"
        )));
    }

    #[test]
    fn file_mutations_require_card_for_person_and_model_not_approved() {
        let calls = vec![
            ToolCall::MoveFile {
                source: "a.txt".into(),
                destination: "b.txt".into(),
            },
            ToolCall::CopyFile {
                source: "a.txt".into(),
                destination: "b.txt".into(),
            },
            ToolCall::RenameFile {
                path: "a.txt".into(),
                new_name: "b.txt".into(),
            },
            ToolCall::CreateFolder("sub".into()),
        ];

        for call in calls {
            assert!(needs_card(&call, Origin::Person));
            assert!(needs_card(&call, Origin::Model));
            assert!(!needs_card(&call, Origin::Approved));
        }
    }
}
