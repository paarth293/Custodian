//! Approved file operations share durable receipts, verification and inverse checks.
use super::*;
use custodian_core::{
    coordinator::TaskState,
    file_tasks::{FileAction, FileTaskPlan, FileTasks, ReceiptState},
};

/// Resolve once, before showing a card. Execution uses this exact snapshot.
pub(super) fn prepare_call(
    app: &AppHandle,
    state: &AssistantState,
    call: &ToolCall,
) -> Result<Option<FileTaskPlan>, String> {
    let (action, source, destination) = match call {
        ToolCall::MoveFile {
            source,
            destination,
        }
        | ToolCall::CopyFile {
            source,
            destination,
        } => {
            let source = resolve_source(app, state, source)?;
            let destination = resolve_dest_dir(app, destination)
                .map(|folder| folder.join(source.file_name().unwrap_or_default()))
                .unwrap_or_else(|| PathBuf::from(destination));
            let action = if matches!(call, ToolCall::MoveFile { .. }) {
                FileAction::Move
            } else {
                FileAction::Copy
            };
            (action, Some(source), destination)
        }
        ToolCall::RenameFile { path, new_name } => {
            let source = resolve_source(app, state, path)?;
            let name = new_name.trim();
            if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
                return Err("That is not a valid file name.".into());
            }
            let destination = source
                .parent()
                .ok_or("A root cannot be renamed")?
                .join(name);
            (FileAction::Rename, Some(source), destination)
        }
        ToolCall::CreateFolder(path) => (FileAction::CreateFolder, None, PathBuf::from(path)),
        _ => return Ok(None),
    };
    if !destination.is_absolute() {
        return Err("Give the full destination path, or name a known folder such as Downloads. Nothing was changed.".into());
    }
    let plan = FileTaskPlan::prepare(action, source.as_deref(), &destination)
        .map_err(|err| err.to_string())?;
    if is_protected_system_path(&plan.destination)
        || plan
            .source
            .as_ref()
            .is_some_and(|path| is_protected_system_path(path))
    {
        return Err("Custodian cannot change files in protected system locations.".into());
    }
    Ok(Some(plan))
}

fn resolve_source(
    app: &AppHandle,
    state: &AssistantState,
    source: &str,
) -> Result<PathBuf, String> {
    if let Some(result) = state.file_references.resolve(source) {
        return result;
    }
    resolve_existing_file(app, source).ok_or_else(|| {
        "I couldn't identify one ordinary local file. Give its full path; nothing was changed."
            .into()
    })
}

fn store(app: &AppHandle) -> Result<FileTasks, String> {
    let dir = crate::paths::data_dir(app).map_err(|err| err.to_string())?;
    if history_ledger::check(&dir.join("ledger"))? != history_ledger::LedgerStatus::Intact {
        return Err(
            "My history could not be verified, so file changes are paused until it is reviewed."
                .into(),
        );
    }
    Ok(FileTasks::new(dir.join("file-tasks")))
}

fn perform(
    app: &AppHandle,
    state: &AssistantState,
    action: FileAction,
    source: Option<PathBuf>,
    destination: PathBuf,
    channel: Sink<'_>,
) {
    let plan = match FileTaskPlan::prepare(action, source.as_deref(), &destination) {
        Ok(plan) => plan,
        Err(err) => return say(channel, format!("No file change was started: {err}")),
    };
    perform_prepared(app, state, &plan, channel);
}

pub(super) fn perform_prepared(
    app: &AppHandle,
    state: &AssistantState,
    plan: &FileTaskPlan,
    channel: Sink<'_>,
) {
    custodian_core::privacy::with_policy(state.privacy.clone(), || {
        perform_checked(app, state, plan, channel)
    });
}

fn perform_checked(
    app: &AppHandle,
    state: &AssistantState,
    plan: &FileTaskPlan,
    channel: Sink<'_>,
) {
    if custodian_core::privacy::check(&plan.destination).is_err()
        || plan
            .source
            .as_ref()
            .is_some_and(|source| custodian_core::privacy::check(source).is_err())
    {
        return say(channel, custodian_core::privacy::DENIED);
    }
    let action = plan.action;
    let source = plan.source.clone();
    let destination = plan.destination.clone();
    let canonical_destination = destination
        .parent()
        .and_then(|parent| parent.canonicalize().ok())
        .and_then(|parent| destination.file_name().map(|name| parent.join(name)))
        .unwrap_or_else(|| destination.clone());
    let canonical_source = source.as_ref().and_then(|path| path.canonicalize().ok());
    if is_protected_system_path(&destination)
        || is_protected_system_path(&canonical_destination)
        || source
            .as_ref()
            .is_some_and(|path| is_protected_system_path(path))
        || canonical_source
            .as_ref()
            .is_some_and(|path| is_protected_system_path(path))
    {
        return say(
            channel,
            "For safety, Custodian cannot change files in protected system locations.",
        );
    }
    let store = match store(app) {
        Ok(store) => store,
        Err(err) => return say(channel, err),
    };
    let mut resources = vec![destination.clone()];
    if let Some(source) = &source {
        resources.push(source.clone());
    }
    let (task_id, cancel) = match state.coordinator.create_task(
        format!("{action:?}: {}", destination.display()),
        TaskState::Preparing,
        &resources,
    ) {
        Ok(task) => task,
        Err(err) => return say(channel, err.to_string()),
    };
    if cancel.load(Ordering::Relaxed) {
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
    match store.execute_plan(plan) {
        Ok(receipt) => {
            let status_note = state
                .coordinator
                .update_state(
                    &task_id,
                    TaskState::Completed,
                    Some("Content and destination verified; undo receipt saved.".into()),
                )
                .err()
                .map(|err| {
                    format!(
                        "The file task was verified, but its task status could not be saved: {err}"
                    )
                });
            let history_note = record_history(
                app,
                json!({"kind": "file_task", "receipt": receipt.id, "action": receipt.action, "source": receipt.source, "destination": receipt.destination, "bytes": receipt.bytes, "at": now_unix()}),
            );
            let past = match action {
                FileAction::Move => "Moved",
                FileAction::Copy => "Copied",
                FileAction::Rename => "Renamed",
                FileAction::CreateFolder => "Created folder",
            };
            let text = format!("{past}: {}. The result was verified and its undo record was saved. Say \"undo\" to put it back.", receipt.destination.display());
            state.record_context(text.clone());
            say(
                channel,
                with_note(with_note(text, history_note), status_note),
            );
        }
        Err(err) => {
            state
                .coordinator
                .update_state(&task_id, TaskState::RecoveryRequired, Some(err.to_string()))
                .ok();
            say(channel, format!("The file task needs review: {err}. Any prepared recovery record has been kept."));
        }
    }
}

pub(super) fn move_or_copy(
    app: &AppHandle,
    state: &AssistantState,
    action: FileAction,
    source: &str,
    destination: &str,
    channel: Sink<'_>,
) {
    let Some(source) = resolve_existing_file(app, source) else {
        return say(
            channel,
            "I couldn't find that ordinary local file. No files were changed.",
        );
    };
    let destination = resolve_dest_dir(app, destination)
        .map(|folder| folder.join(source.file_name().unwrap_or_default()))
        .unwrap_or_else(|| PathBuf::from(destination));
    perform(app, state, action, Some(source), destination, channel);
}

pub(super) fn rename(
    app: &AppHandle,
    state: &AssistantState,
    path: &str,
    new_name: &str,
    channel: Sink<'_>,
) {
    let Some(source) = resolve_existing_file(app, path) else {
        return say(
            channel,
            "I couldn't find that ordinary local file. No files were changed.",
        );
    };
    let name = new_name.trim();
    if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
        return say(channel, "That is not a valid file name.");
    }
    let Some(parent) = source.parent() else {
        return say(channel, "A root directory cannot be renamed.");
    };
    let destination = parent.join(name);
    perform(
        app,
        state,
        FileAction::Rename,
        Some(source),
        destination,
        channel,
    );
}

pub(super) fn create_folder(
    app: &AppHandle,
    state: &AssistantState,
    path: &str,
    channel: Sink<'_>,
) {
    perform(
        app,
        state,
        FileAction::CreateFolder,
        None,
        PathBuf::from(path),
        channel,
    );
}

pub(super) fn undo_latest_if_newer(
    app: &AppHandle,
    state: &AssistantState,
    channel: Sink<'_>,
) -> bool {
    let store = match store(app) {
        Ok(store) => store,
        Err(err) => {
            say(channel, err);
            return true;
        }
    };
    let receipts = match store.receipts() {
        Ok(receipts) => receipts,
        Err(err) => {
            say(
                channel,
                format!("I couldn't verify the file-task records: {err}"),
            );
            return true;
        }
    };
    let latest = receipts
        .into_iter()
        .find(|receipt| receipt.state == ReceiptState::Completed);
    let cleanup_order = quarantine(app)
        .ok()
        .and_then(|quarantine| quarantine.latest_undoable_order())
        .unwrap_or(0);
    let Some(receipt) = latest.filter(|receipt| receipt.created_order >= cleanup_order) else {
        return false;
    };
    if custodian_core::privacy::check(&receipt.destination).is_err()
        || receipt
            .source
            .as_ref()
            .is_some_and(|source| custodian_core::privacy::check(source).is_err())
    {
        say(channel, custodian_core::privacy::DENIED);
        return true;
    }
    let mut resources = vec![receipt.destination.clone()];
    if let Some(source) = &receipt.source {
        resources.push(source.clone());
    }
    let (task_id, _) = match state.coordinator.create_task(
        format!("Undo {}", receipt.destination.display()),
        TaskState::Running,
        &resources,
    ) {
        Ok(task) => task,
        Err(err) => {
            say(channel, err.to_string());
            return true;
        }
    };
    match store.undo(&receipt.id) {
        Ok(undone) => {
            state
                .coordinator
                .update_state(&task_id, TaskState::Completed, None)
                .ok();
            let note = record_history(
                app,
                json!({"kind": "undo_file_task", "receipt": undone.id, "at": now_unix()}),
            );
            let text = format!(
                "Undid the verified file task for {}. No existing files were overwritten.",
                undone.destination.display()
            );
            state.record_context(text.clone());
            say(channel, with_note(text, note));
        }
        Err(err) => {
            state
                .coordinator
                .update_state(&task_id, TaskState::Failed, Some(err.to_string()))
                .ok();
            say(channel, format!("I couldn't undo that file task: {err}"));
        }
    }
    true
}
