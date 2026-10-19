//! Native, revocable app authority. Privacy eligibility alone grants no effects.
//! Grants are intentionally not serializable/deserializable or model-facing.
use crate::app_adapter::{ApplicationAction, ApplicationRequest};
use crate::app_privacy::{ApplicationIdentity, ApplicationPrivacyPolicy};
use crate::task_program::{Failure, FailureClass};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AppPermission {
    Open,
    Inspect,
    Edit,
    Save,
    Close,
    Verify,
}

impl AppPermission {
    fn for_action(action: &ApplicationAction) -> Self {
        match action {
            ApplicationAction::OpenBoundDocument => Self::Open,
            ApplicationAction::InspectBoundDocument => Self::Inspect,
            ApplicationAction::EditBoundRange { .. } | ApplicationAction::EditBoundCells { .. } => {
                Self::Edit
            }
            ApplicationAction::SaveBoundDocument => Self::Save,
            ApplicationAction::CloseDocumentNormally => Self::Close,
            ApplicationAction::VerifySavedOutput => Self::Verify,
        }
    }
}

fn edit_scope(action: &ApplicationAction) -> Option<[u8; 32]> {
    if let ApplicationAction::EditBoundCells { .. } = action {
        let mut digest = blake3::Hasher::new();
        digest.update(b"custodian-app-cell-edit-v1");
        digest.update(&serde_json::to_vec(action).ok()?);
        return Some(*digest.finalize().as_bytes());
    }
    if let ApplicationAction::EditBoundRange {
        start,
        end,
        replacement,
    } = action
    {
        let mut digest = blake3::Hasher::new();
        digest.update(b"custodian-app-edit-v1");
        digest.update(&start.to_le_bytes());
        digest.update(&end.to_le_bytes());
        digest.update(replacement.as_bytes());
        Some(*digest.finalize().as_bytes())
    } else {
        None
    }
}

pub struct ApplicationAuthority {
    id: uuid::Uuid,
    state: RwLock<(u64, ApplicationPrivacyPolicy)>,
    storage: Option<crate::app_privacy_store::Storage>,
    unavailable: AtomicBool,
}

impl Default for ApplicationAuthority {
    fn default() -> Self {
        Self::new(ApplicationPrivacyPolicy::default(), 0)
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationPrivacySettings {
    pub revision: u64,
    pub private_executables: Vec<String>,
}

/// Native consent to create one empty dedicated application instance. This
/// cannot authorize document observation, editing, saving or arbitrary launches.
pub struct ApplicationLaunchGrant {
    issuer: uuid::Uuid,
    executable: String,
    revision: u64,
    expires: Instant,
}
impl ApplicationLaunchGrant {
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

impl ApplicationAuthority {
    /// Current privacy eligibility for displaying historical task evidence.
    /// Refreshing this revision never refreshes a grant or authorizes an effect.
    pub fn task_visible(&self, identity: &ApplicationIdentity, document_id: &str) -> bool {
        let Ok(state) = self.state.read() else {
            return false;
        };
        let mut historical = identity.clone();
        historical.privacy_revision = state.0;
        !self.unavailable.load(Ordering::Acquire)
            && state.1.allows_document(&historical, document_id, state.0)
    }
    /// Eligibility before staging/observing a native-bound file. This read-only
    /// check creates no launch or document-effect grant.
    pub fn admit_document_source(
        &self,
        executable: &str,
        document_id: &str,
    ) -> Result<u64, Failure> {
        let state = self.state.read().map_err(|_| {
            Failure::new(
                FailureClass::MissingAuthority,
                "Application privacy unavailable",
            )
        })?;
        if self.unavailable.load(Ordering::Acquire)
            || !state.1.allows_executable(executable)
            || !state.1.document_eligible(document_id)
        {
            return Err(Failure::new(
                FailureClass::MissingAuthority,
                "Application or bound document is private",
            ));
        }
        Ok(state.0)
    }
    pub fn issue_launch_after_native_consent(
        &self,
        executable: &str,
        lifetime: Duration,
    ) -> Result<ApplicationLaunchGrant, &'static str> {
        let state = self
            .state
            .read()
            .map_err(|_| "application authority unavailable")?;
        if self.unavailable.load(Ordering::Acquire)
            || !state.1.allows_executable(executable)
            || lifetime.is_zero()
            || lifetime > Duration::from_secs(1800)
        {
            return Err("application launch is private, unavailable or out of scope");
        }
        Ok(ApplicationLaunchGrant {
            issuer: self.id,
            executable: executable.into(),
            revision: state.0,
            expires: Instant::now() + lifetime,
        })
    }

    pub fn check_launch(
        &self,
        grant: &ApplicationLaunchGrant,
        executable: &str,
    ) -> Result<(), Failure> {
        let denied = || {
            Failure::new(
                FailureClass::MissingAuthority,
                "Application launch consent changed or expired",
            )
        };
        let state = self.state.read().map_err(|_| denied())?;
        if self.unavailable.load(Ordering::Acquire)
            || grant.issuer != self.id
            || grant.revision != state.0
            || Instant::now() >= grant.expires
            || grant.executable != executable
            || !state.1.allows_executable(executable)
        {
            return Err(denied());
        }
        Ok(())
    }

    /// Privacy eligibility for a freshly native-measured target; still not effect authority.
    pub fn admit_native_target(
        &self,
        identity: &ApplicationIdentity,
        document_id: &str,
    ) -> Result<(), Failure> {
        let denied = || {
            Failure::new(
                FailureClass::MissingAuthority,
                "Application target is private, stale or invalid",
            )
        };
        let state = self.state.read().map_err(|_| denied())?;
        if self.unavailable.load(Ordering::Acquire)
            || !state.1.allows_document(identity, document_id, state.0)
        {
            return Err(denied());
        }
        Ok(())
    }
    pub fn new(policy: ApplicationPrivacyPolicy, revision: u64) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            state: RwLock::new((revision, policy)),
            storage: None,
            unavailable: AtomicBool::new(false),
        }
    }

    /// Fixed native data path only. Missing/corrupt required storage blocks
    /// startup. No persisted app grant survives restart (issuer is always new).
    pub fn open(path: std::path::PathBuf) -> Result<Self, &'static str> {
        let (storage, revision, policy) = crate::app_privacy_store::Storage::open(path)
            .map_err(|_| "Application privacy rules could not be verified. Startup was stopped.")?;
        Ok(Self {
            id: uuid::Uuid::new_v4(),
            state: RwLock::new((revision, policy)),
            storage: Some(storage),
            unavailable: AtomicBool::new(false),
        })
    }

    pub fn settings(&self) -> Result<ApplicationPrivacySettings, &'static str> {
        let state = self
            .state
            .read()
            .map_err(|_| "application authority unavailable")?;
        Ok(ApplicationPrivacySettings {
            revision: state.0,
            private_executables: state.1.private_executables(),
        })
    }

    fn commit_policy(
        &self,
        state: &mut (u64, ApplicationPrivacyPolicy),
        policy: ApplicationPrivacyPolicy,
    ) -> Result<u64, &'static str> {
        policy.validate()?;
        let revision = state
            .0
            .checked_add(1)
            .ok_or("application revision exhausted")?;
        if let Some(storage) = &self.storage {
            if storage.save(revision, &policy).is_err() {
                self.unavailable.store(true, Ordering::Release);
                return Err("Application privacy rules could not be saved. Application access is paused until restart.");
            }
        }
        *state = (revision, policy);
        Ok(revision)
    }

    /// UI can change executable exclusions without clearing native session,
    /// window or document exclusions from the same policy.
    pub fn replace_private_executables(
        &self,
        expected_revision: u64,
        values: Vec<String>,
    ) -> Result<u64, &'static str> {
        let mut state = self
            .state
            .write()
            .map_err(|_| "application authority unavailable")?;
        if state.0 != expected_revision || self.unavailable.load(Ordering::Acquire) {
            return Err("Application privacy changed or is unavailable. Reload the controls.");
        }
        let mut policy = state.1.clone();
        policy.replace_executables(values)?;
        self.commit_policy(&mut state, policy)
    }

    /// Replacing policy invalidates every earlier grant. When opened with native
    /// storage, commit must succeed before the live policy/revision is published.
    pub fn replace_policy(
        &self,
        expected_revision: u64,
        policy: ApplicationPrivacyPolicy,
    ) -> Result<u64, &'static str> {
        let mut state = self
            .state
            .write()
            .map_err(|_| "application authority unavailable")?;
        if state.0 != expected_revision || self.unavailable.load(Ordering::Acquire) {
            return Err("application privacy revision changed");
        }
        self.commit_policy(&mut state, policy)
    }

    /// Call only after the host independently authorizes the exact native-bound
    /// document/effect scope. There is deliberately no serde/IPC grant constructor.
    pub fn issue_after_native_consent(
        &self,
        request: &ApplicationRequest,
        permissions: &[AppPermission],
        lifetime: Duration,
    ) -> Result<ApplicationGrant, &'static str> {
        let state = self
            .state
            .read()
            .map_err(|_| "application authority unavailable")?;
        if self.unavailable.load(Ordering::Acquire) {
            return Err("application authority unavailable");
        }
        request.validate(&state.1, state.0)?;
        if permissions.is_empty() || lifetime.is_zero() || lifetime > Duration::from_secs(1800) {
            return Err("invalid application grant bounds");
        }
        if permissions.contains(&AppPermission::Edit) && edit_scope(&request.action).is_none() {
            return Err("edit authority requires a native-bound range and replacement");
        }
        Ok(ApplicationGrant {
            authority_id: self.id,
            identity: request.identity.clone(),
            document_id: request.document_id.clone(),
            document_name: request.document_name.clone(),
            document_version: request.document_version.clone(),
            permissions: permissions.iter().copied().collect(),
            edit_scope: edit_scope(&request.action),
            expires: Instant::now() + lifetime,
            revoked: Arc::new(AtomicBool::new(false)),
        })
    }

    fn authorize(
        &self,
        request: &ApplicationRequest,
        grant: &ApplicationGrant,
    ) -> Result<(), Failure> {
        let denied = |detail| Failure::new(FailureClass::MissingAuthority, detail);
        let state = self
            .state
            .read()
            .map_err(|_| denied("application authority unavailable"))?;
        if self.unavailable.load(Ordering::Acquire) {
            return Err(denied("application authority unavailable"));
        }
        request.validate(&state.1, state.0).map_err(denied)?;
        if grant.authority_id != self.id
            || grant.revoked.load(Ordering::Acquire)
            || Instant::now() >= grant.expires
            || grant.identity != request.identity
            || grant.document_id != request.document_id
            || grant.document_name != request.document_name
            || grant.document_version != request.document_version
            || !grant
                .permissions
                .contains(&AppPermission::for_action(&request.action))
            || (matches!(
                request.action,
                ApplicationAction::EditBoundRange { .. } | ApplicationAction::EditBoundCells { .. }
            ) && grant.edit_scope != edit_scope(&request.action))
        {
            return Err(denied(
                "application grant is missing, stale, expired or out of scope",
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ApplicationGrant {
    authority_id: uuid::Uuid,
    identity: ApplicationIdentity,
    document_id: String,
    document_name: String,
    document_version: String,
    permissions: BTreeSet<AppPermission>,
    edit_scope: Option<[u8; 32]>,
    expires: Instant,
    revoked: Arc<AtomicBool>,
}

impl ApplicationGrant {
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }
}

/// Cooperative checks only. A supervised worker is still required to enforce a
/// hard deadline on hung API/UIA calls; this object cannot terminate a thread.
pub struct ApplicationControl<'a> {
    authority: &'a ApplicationAuthority,
    grant: &'a ApplicationGrant,
    request: &'a ApplicationRequest,
    cancel: &'a AtomicBool,
    deadline: Instant,
}

impl<'a> ApplicationControl<'a> {
    pub(crate) fn new(
        authority: &'a ApplicationAuthority,
        grant: &'a ApplicationGrant,
        request: &'a ApplicationRequest,
        cancel: &'a AtomicBool,
    ) -> Result<Self, Failure> {
        // Validate timeout bounds before constructing an Instant from untrusted
        // input, and reject before invoking even metadata-only adapter methods.
        authority.authorize(request, grant)?;
        Ok(Self {
            authority,
            grant,
            request,
            cancel,
            deadline: Instant::now() + Duration::from_millis(request.timeout_ms),
        })
    }

    /// Adapters must check immediately before each observation/effect, then use
    /// target-scoped APIs. Revocation does not retroactively undo an in-flight save.
    pub fn check(&self) -> Result<(), Failure> {
        if self.cancel.load(Ordering::Acquire) {
            return Err(Failure::new(
                FailureClass::Unrecoverable,
                "application work stopped",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(Failure::new(
                FailureClass::Unrecoverable,
                "application deadline elapsed",
            ));
        }
        self.authority.authorize(self.request, self.grant)
    }

    pub fn remaining(&self) -> Result<Duration, Failure> {
        self.check()?;
        Ok(self.deadline.saturating_duration_since(Instant::now()))
    }

    pub fn deadline_elapsed(&self) -> bool {
        Instant::now() >= self.deadline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn persisted_exclusions_survive_restart_and_never_restore_grants() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("app-privacy.dpapi");
        let a = ApplicationAuthority::open(path.clone()).unwrap();
        let mut req = request();
        req.identity.privacy_revision = 0;
        let old = grant(&a, &req);
        assert_eq!(
            a.replace_private_executables(0, vec![req.identity.executable.clone()])
                .unwrap(),
            1
        );
        assert!(
            execute_checked(&req, &a, &old, &AtomicBool::new(false), &mut NeverMeasure).is_err()
        );
        assert!(a.replace_private_executables(0, vec![]).is_err());
        let sealed = std::fs::read(&path).unwrap();
        assert!(!sealed
            .windows(b"editor.exe".len())
            .any(|w| w == b"editor.exe"));
        drop(a);
        let a = ApplicationAuthority::open(path.clone()).unwrap();
        assert_eq!(a.settings().unwrap().revision, 1);
        req.identity.privacy_revision = 1;
        assert!(a
            .issue_after_native_consent(&req, &[AppPermission::Save], Duration::from_secs(30))
            .is_err());
        a.replace_private_executables(1, vec![]).unwrap();
        req.identity.privacy_revision = 2;
        let previous_host_grant = grant(&a, &req);
        drop(a);
        let a = ApplicationAuthority::open(path).unwrap();
        assert!(a.authorize(&req, &previous_host_grant).is_err());
        assert!(a.authorize(&req, &grant(&a, &req)).is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn native_window_session_document_rules_survive_executable_only_ui_update() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("policy.dpapi");
        let a = ApplicationAuthority::open(path.clone()).unwrap();
        let mut req = request();
        let mut policy = ApplicationPrivacyPolicy::default();
        policy
            .set_private_windows(std::slice::from_ref(&req.identity))
            .unwrap();
        policy
            .set_private_documents(std::slice::from_ref(&req.document_id))
            .unwrap();
        policy.set_private_processes(["private-session"]);
        a.replace_policy(0, policy).unwrap();
        a.replace_private_executables(1, vec![r"C:\Other\hidden.exe".into()])
            .unwrap();
        drop(a);
        let a = ApplicationAuthority::open(path).unwrap();
        req.identity.privacy_revision = 2;
        assert!(a
            .issue_after_native_consent(&req, &[AppPermission::Save], Duration::from_secs(30))
            .is_err());
        req.identity.window_generation = "different-window".into();
        assert!(a
            .issue_after_native_consent(&req, &[AppPermission::Save], Duration::from_secs(30))
            .is_err());
        req.document_id = "public-document".into();
        assert!(a.authorize(&req, &grant(&a, &req)).is_ok());
        req.identity.process_generation = "private-session".into();
        assert!(a
            .issue_after_native_consent(&req, &[AppPermission::Save], Duration::from_secs(30))
            .is_err());
    }

    #[test]
    #[cfg(windows)]
    fn failed_storage_commit_revokes_live_access_and_preserves_previous_disk_policy() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("private-canary.dpapi");
        let a = ApplicationAuthority::open(path.clone()).unwrap();
        let mut req = request();
        req.identity.privacy_revision = 0;
        let token = grant(&a, &req);
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        let error = a
            .replace_private_executables(0, vec![req.identity.executable.clone()])
            .unwrap_err();
        assert!(!error.contains("canary"));
        assert!(a.authorize(&req, &token).is_err());
        assert!(a
            .issue_after_native_consent(&req, &[AppPermission::Save], Duration::from_secs(30))
            .is_err());
        assert_eq!(a.settings().unwrap().revision, 0);
        drop(held);
        drop(a);
        let a = ApplicationAuthority::open(path).unwrap();
        assert_eq!(a.settings().unwrap().revision, 0);
        assert!(a.settings().unwrap().private_executables.is_empty());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 3); // policy, marker, lock
    }

    #[test]
    #[cfg(windows)]
    fn exclusive_store_and_competing_revisions_prevent_lost_updates() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("policy.dpapi");
        let a = Arc::new(ApplicationAuthority::open(path.clone()).unwrap());
        assert!(ApplicationAuthority::open(path.clone()).is_err());
        let outcomes = std::thread::scope(|s| {
            let first =
                s.spawn(|| a.replace_private_executables(0, vec![r"C:\First\app.exe".into()]));
            let second =
                s.spawn(|| a.replace_private_executables(0, vec![r"C:\Second\app.exe".into()]));
            [first.join().unwrap(), second.join().unwrap()]
        });
        assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
        let snapshot = a.settings().unwrap();
        drop(a);
        let restarted = ApplicationAuthority::open(path)
            .unwrap()
            .settings()
            .unwrap();
        assert_eq!(restarted.revision, 1);
        assert_eq!(snapshot.private_executables, restarted.private_executables);
    }
    use crate::app_adapter::{execute_checked, ApplicationExecutor, ApplicationReceipt};
    use std::cell::Cell;

    fn request() -> ApplicationRequest {
        ApplicationRequest {
            identity: ApplicationIdentity {
                executable: r"C:\Tools\editor.exe".into(),
                pid: 42,
                process_generation: "native-process-42-1".into(),
                window: 28,
                window_generation: "native-window-28-1".into(),
                privacy_revision: 7,
            },
            document_id: "native-doc-1".into(),
            document_name: "native-document-1".into(),
            document_version: "native-version-1".into(),
            action: ApplicationAction::SaveBoundDocument,
            timeout_ms: 10_000,
        }
    }

    fn authority() -> ApplicationAuthority {
        ApplicationAuthority::new(ApplicationPrivacyPolicy::default(), 7)
    }

    #[test]
    fn history_visibility_can_return_without_reviving_old_effect_authority() {
        let a = authority();
        let req = request();
        let grant = grant(&a, &req);
        assert!(a.task_visible(&req.identity, &req.document_id));
        a.replace_private_executables(7, vec![req.identity.executable.clone()])
            .unwrap();
        assert!(!a.task_visible(&req.identity, &req.document_id));
        a.replace_private_executables(8, vec![]).unwrap();
        assert!(a.task_visible(&req.identity, &req.document_id));
        assert!(a.authorize(&req, &grant).is_err());
    }

    #[test]
    fn launch_consent_cannot_cross_issuer_executable_or_privacy_revision() {
        let a = authority();
        let other = authority();
        let executable = r"C:\Tools\editor.exe";
        let launch = a
            .issue_launch_after_native_consent(executable, Duration::from_secs(30))
            .unwrap();
        assert!(a.check_launch(&launch, executable).is_ok());
        assert!(other.check_launch(&launch, executable).is_err());
        assert!(a.check_launch(&launch, r"C:\Tools\other.exe").is_err());
        a.replace_private_executables(7, vec![executable.into()])
            .unwrap();
        assert!(a.check_launch(&launch, executable).is_err());
        assert!(a
            .issue_launch_after_native_consent(executable, Duration::from_secs(30))
            .is_err());
        assert!(a
            .issue_launch_after_native_consent(r"C:\Tools\other.exe", Duration::ZERO)
            .is_err());
    }

    #[test]
    fn cell_edit_grant_binds_exact_sheet_coordinates_values_and_order() {
        use crate::app_adapter::{CellEdit, CellInput};
        let a = authority();
        let mut req = request();
        let first = CellEdit {
            row: 1,
            col: 1,
            value: CellInput::Number {
                decimal: "7".into(),
            },
        };
        let second = CellEdit {
            row: 2,
            col: 1,
            value: CellInput::Text {
                value: "=SUM(A1:A2)".into(),
            },
        };
        req.action = ApplicationAction::EditBoundCells {
            sheet_id: "sheet-native-1".into(),
            cells: vec![first.clone(), second.clone()],
        };
        let grant = a
            .issue_after_native_consent(&req, &[AppPermission::Edit], Duration::from_secs(30))
            .unwrap();
        assert!(a.authorize(&req, &grant).is_ok());
        for (sheet_id, cells) in [
            ("sheet-native-2", vec![first.clone(), second.clone()]),
            ("sheet-native-1", vec![second.clone(), first.clone()]),
            (
                "sheet-native-1",
                vec![
                    CellEdit {
                        row: 3,
                        ..first.clone()
                    },
                    second.clone(),
                ],
            ),
            (
                "sheet-native-1",
                vec![
                    CellEdit {
                        value: CellInput::Number {
                            decimal: "8".into(),
                        },
                        ..first
                    },
                    second,
                ],
            ),
        ] {
            let mut changed = req.clone();
            changed.action = ApplicationAction::EditBoundCells {
                sheet_id: sheet_id.into(),
                cells,
            };
            assert!(a.authorize(&changed, &grant).is_err());
        }
    }

    fn grant(authority: &ApplicationAuthority, request: &ApplicationRequest) -> ApplicationGrant {
        authority
            .issue_after_native_consent(request, &[AppPermission::Save], Duration::from_secs(30))
            .unwrap()
    }

    struct NeverMeasure;
    impl ApplicationExecutor for NeverMeasure {
        fn remeasure(
            &mut self,
            _: &ApplicationRequest,
            _: &ApplicationControl<'_>,
        ) -> Result<ApplicationRequest, Failure> {
            panic!("denied work reached target measurement")
        }
        fn execute(
            &mut self,
            _: &ApplicationRequest,
            _: &ApplicationControl<'_>,
        ) -> Result<ApplicationReceipt, Failure> {
            panic!("denied work reached execution")
        }
    }

    #[test]
    fn privacy_eligibility_does_not_grant_save_permission_or_cross_authority_access() {
        let a = authority();
        let req = request();
        let inspect = a
            .issue_after_native_consent(&req, &[AppPermission::Inspect], Duration::from_secs(30))
            .unwrap();
        let another_host = authority();
        for (host, token) in [(&a, &inspect), (&another_host, &grant(&a, &req))] {
            let error = execute_checked(
                &req,
                host,
                token,
                &AtomicBool::new(false),
                &mut NeverMeasure,
            )
            .unwrap_err();
            assert_eq!(error.class, FailureClass::MissingAuthority);
            assert!(error.recovery_receipt.is_none());
        }
    }

    #[test]
    fn grant_cannot_be_reused_for_another_document_version_process_or_window() {
        let a = authority();
        let req = request();
        let token = grant(&a, &req);
        let mut alternatives = vec![req.clone(); 7];
        alternatives[0].document_name = "unrelated-document".into();
        alternatives[1].document_version = "user-edited-version".into();
        alternatives[2].identity.process_generation = "reused-pid".into();
        alternatives[3].identity.window_generation = "reused-window".into();
        alternatives[4].action = ApplicationAction::CloseDocumentNormally;
        alternatives[5].timeout_ms = u64::MAX;
        // Identical title/version in another tab is still another document.
        alternatives[6].document_id = "different-native-document".into();
        for other in alternatives {
            assert!(execute_checked(
                &other,
                &a,
                &token,
                &AtomicBool::new(false),
                &mut NeverMeasure
            )
            .is_err());
        }
    }

    #[test]
    fn revocation_and_expiry_block_before_measurement_and_apply_to_grant_clones() {
        let a = authority();
        let req = request();
        let token = grant(&a, &req);
        token.clone().revoke();
        assert!(
            execute_checked(&req, &a, &token, &AtomicBool::new(false), &mut NeverMeasure).is_err()
        );
        let mut expired = grant(&a, &req);
        expired.expires = Instant::now() - Duration::from_secs(1);
        assert!(execute_checked(
            &req,
            &a,
            &expired,
            &AtomicBool::new(false),
            &mut NeverMeasure
        )
        .is_err());
        assert!(a
            .issue_after_native_consent(&req, &[], Duration::from_secs(1))
            .is_err());
        assert!(a
            .issue_after_native_consent(&req, &[AppPermission::Save], Duration::ZERO)
            .is_err());
    }

    #[test]
    fn privacy_revision_replacement_invalidates_grants_and_rejects_stale_updates() {
        let a = authority();
        let req = request();
        let token = grant(&a, &req);
        let mut private = ApplicationPrivacyPolicy::default();
        private.set_private_executables([req.identity.executable.as_str()]);
        assert_eq!(a.replace_policy(7, private), Ok(8));
        assert!(a
            .replace_policy(7, ApplicationPrivacyPolicy::default())
            .is_err());
        assert!(
            execute_checked(&req, &a, &token, &AtomicBool::new(false), &mut NeverMeasure).is_err()
        );
        let mut updated = req;
        updated.identity.privacy_revision = 8;
        assert!(a
            .issue_after_native_consent(&updated, &[AppPermission::Save], Duration::from_secs(30))
            .is_err());
    }

    #[test]
    fn elapsed_cooperative_deadline_is_detected_without_sleep_or_effect() {
        let a = authority();
        let req = request();
        let token = grant(&a, &req);
        let cancel = AtomicBool::new(false);
        let mut control = ApplicationControl::new(&a, &token, &req, &cancel).unwrap();
        control.deadline = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            control.check().unwrap_err().class,
            FailureClass::Unrecoverable
        );
    }

    #[test]
    fn unqualified_adapter_cannot_execute_without_native_remeasurement() {
        struct Unqualified;
        impl ApplicationExecutor for Unqualified {
            fn execute(
                &mut self,
                _: &ApplicationRequest,
                _: &ApplicationControl<'_>,
            ) -> Result<ApplicationReceipt, Failure> {
                panic!("an unqualified adapter reached execution")
            }
        }
        let a = authority();
        let req = request();
        let token = grant(&a, &req);
        let error = execute_checked(&req, &a, &token, &AtomicBool::new(false), &mut Unqualified)
            .unwrap_err();
        assert_eq!(error.class, FailureClass::UnavailableCapability);
        assert!(error.recovery_receipt.is_none());
    }

    #[test]
    fn edit_authority_cannot_expand_the_range_or_change_the_authorized_replacement() {
        let a = authority();
        let mut req = request();
        assert!(a
            .issue_after_native_consent(&req, &[AppPermission::Edit], Duration::from_secs(30))
            .is_err());
        req.action = ApplicationAction::EditBoundRange {
            start: 3,
            end: 8,
            replacement: "approved text".into(),
        };
        let token = a
            .issue_after_native_consent(&req, &[AppPermission::Edit], Duration::from_secs(30))
            .unwrap();
        let cancel = AtomicBool::new(false);
        assert!(ApplicationControl::new(&a, &token, &req, &cancel)
            .unwrap()
            .check()
            .is_ok());
        for action in [
            ApplicationAction::EditBoundRange {
                start: 0,
                end: 8,
                replacement: "approved text".into(),
            },
            ApplicationAction::EditBoundRange {
                start: 3,
                end: 8,
                replacement: "different text".into(),
            },
        ] {
            req.action = action;
            assert!(execute_checked(&req, &a, &token, &cancel, &mut NeverMeasure).is_err());
        }
    }

    struct MeasuredFixture<'a> {
        token: &'a ApplicationGrant,
        authority: &'a ApplicationAuthority,
        cancel: &'a AtomicBool,
        mode: u8,
        measured: Cell<usize>,
        effects: Cell<usize>,
        output: std::path::PathBuf,
    }

    impl ApplicationExecutor for MeasuredFixture<'_> {
        fn remeasure(
            &mut self,
            req: &ApplicationRequest,
            control: &ApplicationControl<'_>,
        ) -> Result<ApplicationRequest, Failure> {
            control.check()?;
            self.measured.set(self.measured.get() + 1);
            let mut measured = req.clone();
            match self.mode {
                1 => measured.document_version = "changed-between-checks".into(),
                2 => self.token.revoke(),
                3 => {
                    self.authority
                        .replace_policy(7, ApplicationPrivacyPolicy::default())
                        .unwrap();
                }
                _ => {}
            }
            Ok(measured)
        }
        fn execute(
            &mut self,
            req: &ApplicationRequest,
            control: &ApplicationControl<'_>,
        ) -> Result<ApplicationReceipt, Failure> {
            control.check()?;
            self.effects.set(self.effects.get() + 1);
            std::fs::write(&self.output, b"synthetic verified effect").unwrap();
            match self.mode {
                4 => self.token.revoke(),
                5 => self.cancel.store(true, Ordering::Release),
                6 => {
                    self.authority
                        .replace_policy(7, ApplicationPrivacyPolicy::default())
                        .unwrap();
                }
                _ => {}
            }
            Ok(ApplicationReceipt {
                identity: req.identity.clone(),
                document_id: req.document_id.clone(),
                document_name: req.document_name.clone(),
                action: req.action.clone(),
                verified: self.mode != 7,
                detail: "synthetic effect evidence; not a real app".into(),
            })
        }
    }

    #[test]
    fn fresh_target_and_authority_are_rechecked_after_measurement_before_any_effect() {
        for mode in 1..=3 {
            let a = authority();
            let req = request();
            let token = grant(&a, &req);
            let cancel = AtomicBool::new(false);
            let dir = tempfile::tempdir().unwrap();
            let mut adapter = MeasuredFixture {
                token: &token,
                authority: &a,
                cancel: &cancel,
                mode,
                measured: Cell::new(0),
                effects: Cell::new(0),
                output: dir.path().join("must-not-exist.txt"),
            };
            let error = execute_checked(&req, &a, &token, &cancel, &mut adapter).unwrap_err();
            assert_eq!(adapter.measured.get(), 1);
            assert_eq!(adapter.effects.get(), 0);
            assert!(!adapter.output.exists());
            assert!(error.recovery_receipt.is_none());
        }
    }

    #[test]
    fn post_effect_stop_revocation_and_unverified_results_preserve_real_effect_evidence() {
        for mode in 0..=7 {
            if (1..=3).contains(&mode) {
                continue;
            }
            let a = authority();
            let req = request();
            let token = grant(&a, &req);
            let cancel = AtomicBool::new(false);
            let dir = tempfile::tempdir().unwrap();
            let mut adapter = MeasuredFixture {
                token: &token,
                authority: &a,
                cancel: &cancel,
                mode,
                measured: Cell::new(0),
                effects: Cell::new(0),
                output: dir.path().join("effect.txt"),
            };
            let result = execute_checked(&req, &a, &token, &cancel, &mut adapter);
            assert_eq!(adapter.effects.get(), 1);
            assert_eq!(
                std::fs::read(&adapter.output).unwrap(),
                b"synthetic verified effect"
            );
            if mode == 0 {
                assert!(result.is_ok());
            } else {
                let error = result.unwrap_err();
                assert!(error.recovery_receipt.is_some());
                assert!(!format!("{error:?}").contains("synthetic effect evidence"));
            }
        }
    }
}
