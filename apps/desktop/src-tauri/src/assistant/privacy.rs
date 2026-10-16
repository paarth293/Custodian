//! Native privacy controls and the truthful offline capability boundary.
use super::*;
use custodian_core::privacy::{AccessPolicy, OFFLINE};

pub(super) const PAUSED: &str = "This capability is paused until it uses the shared privacy and execution boundary. Nothing was changed.";

pub(super) fn unavailable(call: &ToolCall) -> Option<&'static str> {
    match call {
        ToolCall::WebSearch(_) | ToolCall::OpenWebsite { .. } => Some(OFFLINE),
        // Legacy paths enumerate titles, inspect unscoped content, or mutate
        // through a different recovery contract. Do not bypass policy for them.
        ToolCall::FindDuplicateFiles(_)
        | ToolCall::CleanTempFiles
        | ToolCall::EmptySafePlace
        | ToolCall::CleanupHistory
        | ToolCall::Suggestions
        | ToolCall::ListOpenApps
        | ToolCall::CloseApp(_)
        | ToolCall::OpenProgram(_) => Some(PAUSED),
        _ => None,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySettings {
    revision: u64,
    private_paths: Vec<PathBuf>,
    offline: bool,
    notice: &'static str,
}

fn snapshot(policy: &AccessPolicy) -> PrivacySettings {
    let (revision, private_paths) = policy.settings();
    PrivacySettings {
        revision, private_paths, offline: true,
        notice: "Private locations and common credential files are excluded from supported file tools. Multi-linked files and reparse/cloud paths are refused. App-level private zones and general app control are not implemented. Existing third-party applications are not network-isolated by these controls.",
    }
}

#[tauri::command]
pub fn get_privacy_settings(state: tauri::State<'_, AssistantState>) -> PrivacySettings {
    snapshot(&state.privacy)
}

#[tauri::command]
pub async fn set_private_paths(
    app: AppHandle,
    paths: Vec<PathBuf>,
    revision: u64,
) -> Result<PrivacySettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        if state.privacy.revision() != revision { return Err("Privacy rules changed. Reopen the controls before saving.".into()); }
        state.cancel.store(true, Ordering::Relaxed);
        state.coordinator.cancel_all();
        state.privacy.replace_if_revision(paths, revision).map_err(|_| "Privacy rules could not be saved. Use existing absolute local paths and reload if another window changed the rules.".to_string())?;
        state.coordinator.cancel_all();
        state.actions.clear();
        state.plans.lock().unwrap_or_else(|e| e.into_inner()).clear();
        state.reset_context();
        // A later model turn discards its old process before accepting any
        // context. Active turns are cancelled and their old-generation output
        // is suppressed by the command sink.
        Ok(snapshot(&state.privacy))
    }).await.map_err(|_| "Could not update privacy rules.".to_string())?
}

#[derive(Serialize)]
pub struct Capability {
    name: &'static str,
    status: &'static str,
    detail: &'static str,
}

#[tauri::command]
pub fn get_capabilities() -> Vec<Capability> {
    vec![
        Capability { name: "Local file search and metadata", status: "available", detail: "Accessible local drives and permitted custom paths; private/credential exclusions apply." },
        Capability { name: "Move, copy, rename, create folder and undo", status: "available", detail: "Ordinary local files; exact-path approval, current privacy checks, verification and durable undo. General path-race hardening remains open." },
        Capability { name: "Windows personal settings", status: "limited", detail: "Six catalogued per-user settings; broader system controls are not implemented." },
        Capability { name: "Local chat, reminders and arithmetic", status: "available", detail: "Chat requires the pinned local model; reminders require the app to be running or reopening." },
        Capability { name: "Cleanup and broad app actions", status: "paused", detail: PAUSED },
        Capability { name: "Web, account and online actions", status: "deferred", detail: OFFLINE },
        Capability { name: "Document editing, general app control and multi-step operator", status: "not implemented", detail: "Planned milestones; opening an app is not completing work inside it." },
    ]
}

pub(super) fn capability_text() -> String {
    get_capabilities()
        .iter()
        .map(|c| format!("{} — {}. {}", c.name, c.status, c.detail))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn privacy_changes_remove_old_paths_from_model_context() {
        let root = tempfile::tempdir().unwrap();
        let private = root.path().join("Private");
        std::fs::create_dir(&private).unwrap();
        let state = AssistantState::default();
        state.record_context(format!(
            "A previous permitted observation: {}",
            private.join("canary.txt").display()
        ));
        state.record_context("An unrelated public fact.".into());
        assert!(state.get_context_projection().contains("canary.txt"));
        state
            .privacy
            .replace_private_paths(vec![private.clone()])
            .unwrap();
        assert!(!state.get_context_projection().contains("canary.txt"));
        state.record_context(format!(
            "Late result: {}",
            private.join("canary.txt").display()
        ));
        assert!(!state.get_context_projection().contains("Late result"));
        assert!(state
            .get_context_projection()
            .contains("unrelated public fact"));
        custodian_core::privacy::with_policy(state.privacy.clone(), || {
            state.privacy.replace_private_paths(vec![private]).unwrap();
            state.record_context("Late result without a full path must also be discarded.".into());
            assert!(state.get_context_projection().is_empty());
        });
        assert!(!state
            .get_context_projection()
            .contains("Late result without"));
    }
    #[test]
    fn approved_calls_cannot_bypass_offline_or_paused_capabilities() {
        assert_eq!(
            unavailable(&ToolCall::WebSearch("test".into())),
            Some(OFFLINE)
        );
        assert_eq!(
            unavailable(&ToolCall::OpenWebsite {
                name: "test".into(),
                url: "https://example.invalid".into()
            }),
            Some(OFFLINE)
        );
        assert_eq!(unavailable(&ToolCall::EmptySafePlace), Some(PAUSED));
        assert_eq!(
            unavailable(&ToolCall::OpenProgram("Chrome".into())),
            Some(PAUSED)
        );
        assert!(unavailable(&ToolCall::GetFileInfo("C:\\Work\\report.txt".into())).is_none());
        assert!(capability_text().contains("not implemented"));
    }
}
