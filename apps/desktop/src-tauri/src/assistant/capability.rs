//! Exhaustive native capability contract. Inputs are the validated ToolCall;
//! this gate runs before preparation/observation and again after approval.
//! Resource-specific identity/content checks remain at the native executor's
//! commit boundary. Neither model text nor arguments can construct Origin.
use super::{actions, privacy, Origin, ToolCall};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Effect {
    ReadMetadata,
    Launch,
    ChangeFiles,
    ChangeSetting,
    LocalTasks,
    Report,
    Disabled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Permission {
    DirectRequest,
    ExactApproval,
    PersonOnly,
    Paused,
    Offline,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Resources {
    LocalPaths,
    CataloguedApplication,
    CataloguedSetting,
    AssistantState,
    None,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Verifier {
    MeasuredRead,
    FileIdentityAndContent,
    SettingReadback,
    DispatchOnly,
    PersistedState,
    None,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Recovery {
    RetainReceiptAndVerifyInverse,
    SettingHistory,
    NoReplay,
    None,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CancelBoundary {
    BeforeDispatch,
    FinishCurrentCommitThenStop,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Preconditions {
    LivePrivacyAndCancellation,
    ExactFileSnapshotAndNoOverwrite,
    CataloguedValueAndCurrentSetting,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Contract {
    pub effect: Effect,
    pub permission: Permission,
    pub resources: Resources,
    pub verifier: Verifier,
    pub recovery: Recovery,
    pub cancellation: CancelBoundary,
    pub preconditions: Preconditions,
}

impl Contract {
    pub fn for_call(call: &ToolCall) -> Self {
        use Effect as E;
        use Permission as P;
        use Recovery as U;
        use Resources as R;
        use Verifier as V;
        let (effect, permission, resources, verifier, recovery) = match call {
            ToolCall::MoveFile { .. }
            | ToolCall::CopyFile { .. }
            | ToolCall::RenameFile { .. }
            | ToolCall::CreateFolder(_) => (
                E::ChangeFiles,
                P::ExactApproval,
                R::LocalPaths,
                V::FileIdentityAndContent,
                U::RetainReceiptAndVerifyInverse,
            ),
            ToolCall::UndoLastCleanup => (
                E::ChangeFiles,
                P::DirectRequest,
                R::LocalPaths,
                V::FileIdentityAndContent,
                U::RetainReceiptAndVerifyInverse,
            ),
            ToolCall::ChangeSetting { .. } => (
                E::ChangeSetting,
                P::ExactApproval,
                R::CataloguedSetting,
                V::SettingReadback,
                U::SettingHistory,
            ),
            ToolCall::RevertSettingChange => (
                E::ChangeSetting,
                P::PersonOnly,
                R::CataloguedSetting,
                V::SettingReadback,
                U::SettingHistory,
            ),
            ToolCall::OpenApp(_) | ToolCall::OpenFolder(_) | ToolCall::OpenFile(_) => (
                E::Launch,
                P::DirectRequest,
                if matches!(call, ToolCall::OpenApp(_)) {
                    R::CataloguedApplication
                } else {
                    R::LocalPaths
                },
                V::DispatchOnly,
                U::NoReplay,
            ),
            ToolCall::FindLargeFiles { .. }
            | ToolCall::FindFiles { .. }
            | ToolCall::GetFileInfo(_) => (
                E::ReadMetadata,
                P::DirectRequest,
                R::LocalPaths,
                V::MeasuredRead,
                U::None,
            ),
            ToolCall::CheckDiskSpace | ToolCall::SystemStatus | ToolCall::ListSettings => (
                E::ReadMetadata,
                P::DirectRequest,
                R::None,
                V::MeasuredRead,
                U::None,
            ),
            ToolCall::AddTask { .. } | ToolCall::CompleteTask(_) | ToolCall::ListTasks => (
                E::LocalTasks,
                P::DirectRequest,
                R::AssistantState,
                V::PersistedState,
                U::NoReplay,
            ),
            ToolCall::RemoveTask(_) => (
                E::LocalTasks,
                P::PersonOnly,
                R::AssistantState,
                V::PersistedState,
                U::NoReplay,
            ),
            ToolCall::SecurityStatus => (E::Report, P::DirectRequest, R::None, V::None, U::None),
            ToolCall::WebSearch(_) | ToolCall::OpenWebsite { .. } => {
                (E::Disabled, P::Offline, R::None, V::None, U::None)
            }
            ToolCall::FindDuplicateFiles(_)
            | ToolCall::CleanTempFiles
            | ToolCall::EmptySafePlace
            | ToolCall::CleanupHistory
            | ToolCall::Suggestions
            | ToolCall::ListOpenApps
            | ToolCall::CloseApp(_)
            | ToolCall::OpenProgram(_) => (E::Disabled, P::Paused, R::None, V::None, U::None),
        };
        Self {
            effect,
            permission,
            resources,
            verifier,
            recovery,
            cancellation: if matches!(effect, E::ChangeFiles | E::ChangeSetting) {
                CancelBoundary::FinishCurrentCommitThenStop
            } else {
                CancelBoundary::BeforeDispatch
            },
            preconditions: match effect {
                E::ChangeFiles => Preconditions::ExactFileSnapshotAndNoOverwrite,
                E::ChangeSetting => Preconditions::CataloguedValueAndCurrentSetting,
                _ => Preconditions::LivePrivacyAndCancellation,
            },
        }
    }

    pub fn unavailable(self) -> Option<&'static str> {
        match self.permission {
            Permission::Paused => Some(privacy::PAUSED),
            Permission::Offline => Some(custodian_core::privacy::OFFLINE),
            _ => None,
        }
    }

    pub fn needs_approval(self, call: &ToolCall, origin: Origin) -> bool {
        origin != Origin::Approved
            && (self.permission == Permission::ExactApproval
                || (origin == Origin::Model && actions::card_text(call).is_some()))
    }

    /// Universal preconditions. Executor-specific preconditions (exact object,
    /// current setting value, no overwrite, budgets) are rechecked at commit.
    pub fn check(self, origin: Origin, stopped: bool) -> Result<(), &'static str> {
        if let Some(reason) = self.unavailable() {
            return Err(reason);
        }
        if stopped {
            return Err(super::STOPPED);
        }
        if !custodian_core::privacy::request_is_current() {
            return Err("Privacy rules changed; this request was stopped.");
        }
        if self.permission == Permission::PersonOnly && origin == Origin::Model {
            return Err("Only the person can ask for that; tell them how instead.");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_and_screen_capture_and_scripts_have_no_model_dispatch_route() {
        for route in [
            "capture_screen",
            "read_window",
            "read_accessibility",
            "extract_app_content",
            "run_script",
        ] {
            assert!(ToolCall::parse(route, &serde_json::json!({"consent": true})).is_err());
        }
        assert!(Contract::for_call(&ToolCall::ListOpenApps)
            .check(Origin::Approved, false)
            .is_err());
    }
    #[test]
    fn consent_and_paused_routes_cannot_be_granted_by_model_arguments() {
        for call in [
            ToolCall::MoveFile {
                source: "x".into(),
                destination: "y".into(),
            },
            ToolCall::ChangeSetting {
                setting: "x".into(),
                value: "y".into(),
            },
        ] {
            let contract = Contract::for_call(&call);
            assert!(contract.needs_approval(&call, Origin::Model));
            assert!(contract.needs_approval(&call, Origin::Person));
            assert!(!contract.needs_approval(&call, Origin::Approved));
            assert!(contract.check(Origin::Approved, true).is_err());
        }
        for call in [
            ToolCall::ListOpenApps,
            ToolCall::OpenProgram("Calculator".into()),
            ToolCall::WebSearch("offline bypass".into()),
        ] {
            for origin in [Origin::Model, Origin::Person, Origin::Approved] {
                assert!(Contract::for_call(&call).check(origin, false).is_err());
            }
        }
        assert!(Contract::for_call(&ToolCall::RevertSettingChange)
            .check(Origin::Model, false)
            .is_err());
    }
}
