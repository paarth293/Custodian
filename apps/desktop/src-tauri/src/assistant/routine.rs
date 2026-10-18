//! Native goal execution. A model cannot create a backup request or change its grant.
use super::*;
use custodian_core::{
    coordinator::TaskState,
    routine::{RoutinePolicy, RoutineSettings},
};

#[derive(Debug, PartialEq, Eq)]
struct BackupRequest {
    source: String,
    destination: PathBuf,
}

fn parse_request(message: &str) -> Option<Result<BackupRequest, &'static str>> {
    let trimmed = message.trim();
    let lower = trimmed.to_ascii_lowercase();
    let prefix = [
        "back up the largest file in ",
        "back up largest file in ",
        "backup the largest file in ",
        "backup largest file in ",
    ]
    .into_iter()
    .find(|p| lower.starts_with(p))?;
    let parts: Vec<_> = trimmed[prefix.len()..].split('"').collect();
    if parts.len() != 5
        || !parts[0].trim().is_empty()
        || !parts[4].trim().is_empty()
        || !parts[2].trim().eq_ignore_ascii_case("to")
        || parts[1].is_empty()
        || !Path::new(parts[3]).is_absolute()
        || !(Path::new(parts[1]).is_absolute() || parts[1].eq_ignore_ascii_case("all drives"))
    {
        return Some(Err("Use: Back up the largest file in \"C:\\MyProject\" to \"D:\\Backups\". Both folders must already exist. Use \"all drives\" as the source to search your PC."));
    }
    Some(Ok(BackupRequest {
        source: parts[1].into(),
        destination: parts[3].into(),
    }))
}

#[tauri::command]
pub fn get_routine_settings(state: tauri::State<'_, AssistantState>) -> RoutineSettings {
    state.routine.settings()
}

#[tauri::command]
pub async fn set_routine_settings(
    app: AppHandle,
    revision: u64,
    automatic_backup: bool,
    automatic_collections: Option<bool>,
    source_roots: Option<Vec<PathBuf>>,
    destination_roots: Option<Vec<PathBuf>>,
) -> Result<RoutineSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        state.stop_requests();
        let result = state
            .routine
            .update_scopes(
                revision,
                automatic_backup,
                automatic_collections,
                source_roots,
                destination_roots,
            )
            .map_err(|_| {
                "Routine permissions could not be saved. Reload the controls before trying again."
                    .to_string()
            })?;
        state.coordinator.cancel_all();
        state.actions.clear();
        state.reset_context();
        Ok(result)
    })
    .await
    .map_err(|_| "Routine permissions could not be updated.".to_string())?
}

pub(super) fn eligible(path: &Path) -> bool {
    !is_protected_system_path(path)
        && !path.components().any(|part| {
            let text = part.as_os_str().to_string_lossy().to_ascii_lowercase();
            matches!(
                text.as_str(),
                "onedrive" | "dropbox" | "iclouddrive" | "google drive"
            ) || text.starts_with("onedrive - ")
        })
}

/// Called only from the actual person's deterministic chat entry, before LLM
/// dispatch. Model tool calls have no equivalent of this goal or preferences.
pub(super) fn handle(
    app: &AppHandle,
    state: &AssistantState,
    message: &str,
    channel: Sink<'_>,
    revision: u64,
) -> bool {
    let Some(parsed) = parse_request(message) else {
        return false;
    };
    let request = match parsed {
        Ok(request) => request,
        Err(text) => {
            say(channel, text);
            return true;
        }
    };
    let grant = state.routine.settings();
    if !grant.automatic_backup || grant.revision != revision {
        say(channel, "Automatic backup is off. Enable it once in Routine permissions, then ask again. Existing files will not be replaced.");
        return true;
    }
    if custodian_core::privacy::check(&request.destination).is_err() {
        say(channel, custodian_core::privacy::DENIED);
        return true;
    }
    let scope = if request.source.eq_ignore_ascii_case("all drives") {
        Scope::AllDrives
    } else {
        Scope::Custom(PathBuf::from(&request.source))
    };
    let roots: Vec<_> = resolve_scope(app, &scope)
        .into_iter()
        .map(|(_, path)| path)
        .collect();
    let (task, cancel) = match state.create_task(
        "Preparing a verified largest-file backup".into(),
        TaskState::Running,
        std::slice::from_ref(&request.destination),
    ) {
        Ok(task) => task,
        Err(error) => {
            say(channel, error.to_string());
            return true;
        }
    };
    emit(
        channel,
        ChatEvent::Status {
            text: "Finding the largest accessible, permitted file for backup…".into(),
        },
    );
    let prepared = match custodian_core::routine::prepare_backup(
        &state.routine,
        revision,
        &roots,
        &request.destination,
        &cancel,
        &eligible,
        &mut |count| {
            emit(
                channel,
                ChatEvent::Status {
                    text: format!("Backup: checked {count} files…"),
                },
            )
        },
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            let status = if error.kind() == std::io::ErrorKind::Interrupted {
                TaskState::Cancelled
            } else {
                TaskState::Failed
            };
            state.coordinator.update_state(&task, status, None).ok();
            say(channel, format!("Backup did not start: {error}"));
            return true;
        }
    };
    let store = match file_ops::store(app) {
        Ok(store) => store,
        Err(error) => {
            state
                .coordinator
                .update_state(&task, TaskState::Failed, None)
                .ok();
            say(channel, error);
            return true;
        }
    };
    emit(
        channel,
        ChatEvent::Status {
            text: "Creating and verifying the backup…".into(),
        },
    );
    match custodian_core::routine::execute_backup(&state.routine, &prepared, &store, &cancel) {
        Ok(receipt) => {
            let note = state
                .coordinator
                .update_state(
                    &task,
                    TaskState::Completed,
                    Some("Backup content and durable undo verified.".into()),
                )
                .err()
                .map(|_| "The backup completed, but its task status could not be saved.".into());
            let history = record_history(
                app,
                json!({"kind":"routine_backup", "receipt":receipt.id,
                "source":receipt.source, "destination":receipt.destination, "bytes":receipt.bytes, "at":now_unix()}),
            );
            let text = format!("Backup verified: {} ({} bytes). Selected the largest accessible, permitted file from {} files checked; {} locations were inaccessible. The original is unchanged and the undo record was saved. Say \"undo\" to remove this verified copy.",
                receipt.destination.display(), receipt.bytes, prepared.stats.files_seen, prepared.stats.access_denied_count);
            state.record_context(text.clone());
            say(channel, with_note(with_note(text, history), note));
        }
        Err(error) => {
            state
                .coordinator
                .update_state(&task, TaskState::RecoveryRequired, None)
                .ok();
            say(channel, format!("No successful backup was verified: {error}. Existing files were not overwritten. Any prepared recovery record and partial copy have been retained for review."));
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_goal_requires_explicit_absolute_locations_and_never_sets_permissions() {
        let request =
            parse_request("Back up the largest file in \"C:\\Projects\" to \"D:\\Backups\"")
                .unwrap()
                .unwrap();
        assert_eq!(request.source, "C:\\Projects");
        assert!(
            parse_request("Back up the largest file in \"all drives\" to \"D:\\Backups\"")
                .unwrap()
                .is_ok()
        );
        assert!(
            parse_request("Back up the largest file in Documents to Downloads")
                .unwrap()
                .is_err()
        );
        assert!(
            parse_request("Back up the largest file in \"C:\\Projects\" to \"relative\"")
                .unwrap()
                .is_err()
        );
        assert!(parse_request("Enable automatic backup").is_none());
        assert!(parse_request("Clean up my computer").is_none());
    }
}
