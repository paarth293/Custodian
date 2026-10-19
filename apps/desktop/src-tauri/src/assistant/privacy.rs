//! Native privacy controls and the truthful offline capability boundary.
use super::*;
use custodian_core::privacy::{AccessPolicy, OFFLINE};

pub(super) const PAUSED: &str = "This capability is paused until it uses the shared privacy and execution boundary. Nothing was changed.";

pub(super) fn unavailable(call: &ToolCall) -> Option<&'static str> {
    capability::Contract::for_call(call).unavailable()
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
        notice: "Private locations and common credential files are excluded from supported file tools. Multi-linked files and reparse/cloud paths are refused. Qualified editor and spreadsheet tasks use only their own bound document through dedicated application APIs. Screen capture, general accessibility reading and window-title enumeration remain disabled. Existing third-party applications are not network-isolated by these controls.",
    }
}

#[tauri::command]
pub fn get_privacy_settings(state: tauri::State<'_, AssistantState>) -> PrivacySettings {
    snapshot(&state.privacy)
}

#[tauri::command]
pub fn get_app_privacy_settings(
    state: tauri::State<'_, AssistantState>,
) -> Result<custodian_core::app_authority::ApplicationPrivacySettings, String> {
    state.app_privacy.settings().map_err(str::to_string)
}

#[tauri::command]
pub async fn set_private_applications(
    app: AppHandle,
    executables: Vec<String>,
    revision: u64,
) -> Result<custodian_core::app_authority::ApplicationPrivacySettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AssistantState>();
        state.stop_requests();
        let result = state
            .app_privacy
            .replace_private_executables(revision, executables);
        // Suppress old-generation model output even if file rules did not change.
        state.reset_context();
        state.actions.clear();
        state
            .plans
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        result.map_err(str::to_string)?;
        state.app_privacy.settings().map_err(str::to_string)
    })
    .await
    .map_err(|_| "Application privacy could not be updated.".to_string())?
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
        state.stop_requests();
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
        Capability { name: "Model-planned verified collections", status: "limited", detail: "Qwen proposes typed discovery/filter/select/copy/verify steps. Native goal and standing collection permission bind paths, intent and budgets. Up to 20 files/100 MiB, sealed per-step receipts, restart inspection and group undo. Document/image filters use extensions; no content-based secret detection or archives." },
        Capability { name: "Verified largest-file backup", status: "limited", detail: "Native find/select/copy/verify workflow with opt-in standing permission, 100 MiB limit, free-space checks and durable undo. Explicit local source and destination; no model permission grants." },
        Capability { name: "Local file search and metadata", status: "available", detail: "Accessible local drives and permitted custom paths; private/credential exclusions apply." },
        Capability { name: "Move, copy, rename, create folder and undo", status: "available", detail: "Ordinary local files; exact-path approval, current privacy checks, verification and durable undo. General path-race hardening remains open." },
        Capability { name: "Windows personal settings", status: "limited", detail: "Six catalogued per-user settings; broader system controls are not implemented." },
        Capability { name: "Local chat, reminders and arithmetic", status: "available", detail: "Chat requires the pinned local model; reminders require the app to be running or reopening." },
        Capability { name: "Cleanup and broad app actions", status: "paused", detail: PAUSED },
        Capability { name: "Web, account and online actions", status: "deferred", detail: OFFLINE },
        Capability { name: "Qualified local application tasks", status: "limited", detail: "One-task approval for explicit UTF-8 .txt replacements in VS Code 1.140.0 or single-sheet Excel 16.0.20430.20146 edits in A1:P16. Dedicated instances, typed values and bounded SUM, saved-file verification, normal close, new-output publication and receipt-backed undo. Original files remain intact. Other apps, macros, external links, overwrites, global input and visual fallback are unavailable." },
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
