//! Authority-bound recycle and cleanup operations.
//!
//! Recycle/cleanup is deliberately separate from copy, backup and restore
//! authority. This module never calls `remove_file`, `remove_dir` or an
//! equivalent permanent-delete primitive. A native provider must establish a
//! supported recovery mechanism and independently verify the resulting
//! receipt; unsupported or recovery-unverified providers are refused.
//!
//! The provider integration point is intentionally explicit. The Windows
//! implementation must bind this trait to a real Recycle Bin/provider identity
//! and recovery receipt before exporting the module from `lib.rs`. A generic
//! filesystem move is not a substitute for that provider.
//!
//! `RecoveryReceipt` is the native provider's recovery record and is returned
//! with every verified effect. The host integration must persist that record
//! in its existing durable journal before presenting the operation as
//! recoverable; this module does not invent a path-based fallback or claim
//! recovery from an unverified provider.

use custodian_approval::{Approval, ApprovalError, PlanHash, Verifier};
use custodian_winsec::fsops::{file_identity, is_local_volume, FileIdentity};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const MAX_TARGETS: usize = 50_000;

#[derive(Debug, Error)]
pub enum RecycleError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("recycle manifest error: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("recycle authority refused the plan: {0}")]
    NotAuthorized(#[from] ApprovalError),
    #[error("invalid recycle request: {0}")]
    Invalid(String),
    #[error("recycle/recovery provider is unsupported: {0}")]
    Unsupported(String),
    #[error("recovery could not be independently verified: {0}")]
    RecoveryUnverified(String),
    #[error("target changed since the plan was prepared")]
    TargetChanged,
    #[error("restore destination changed since the plan was prepared")]
    RestoreDestinationChanged,
}

/// Provider capability is checked before an authority can be exercised. Both
/// recycle and restore verification are mandatory; a provider that can only
/// move or unlink data is not eligible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryCapability {
    pub provider_id: String,
    pub supports_recycle: bool,
    pub supports_restore: bool,
    pub verifies_recycle: bool,
    pub verifies_restore: bool,
}

impl RecoveryCapability {
    fn supported(&self) -> bool {
        !self.provider_id.is_empty()
            && self.supports_recycle
            && self.supports_restore
            && self.verifies_recycle
            && self.verifies_restore
    }
}

/// A native provider owns the consequential operation. Its `recycle` method
/// must place the object in a recoverable provider-owned location, never
/// permanently unlink it, and return a receipt that `verify_recycled` can
/// inspect independently.
pub trait RecoveryProvider {
    fn capability(&self) -> RecoveryCapability;
    fn recycle(&self, target: &RecycleTarget) -> Result<RecoveryReceipt, RecycleError>;
    fn verify_recycled(&self, receipt: &RecoveryReceipt) -> Result<(), RecycleError>;
    fn restore(&self, receipt: &RecoveryReceipt, destination: &Path) -> Result<(), RecycleError>;
    fn verify_restored(
        &self,
        receipt: &RecoveryReceipt,
        destination: &Path,
    ) -> Result<(), RecycleError>;
}

/// A provider-independent target snapshot. Approval signs all fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecycleTarget {
    pub path: PathBuf,
    pub identity: FileIdentity,
    pub size: u64,
    pub modified_nanos: u128,
    pub hash: String,
}

/// Native recovery identity. The token is opaque to the caller and is not a
/// path that the module will unlink or interpret as an instruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryReceipt {
    pub provider_id: String,
    pub token: String,
    pub original_path: PathBuf,
    pub identity: FileIdentity,
    pub size: u64,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecyclePlan {
    pub id: String,
    pub provider_id: String,
    pub targets: Vec<RecycleTarget>,
}

impl RecyclePlan {
    pub fn digest(&self) -> PlanHash {
        digest_json(b"custodian-governed-recycle-v1", self)
    }
}

/// Cleanup evidence is typed so age or size by itself cannot become a delete
/// decision. The native planner must supply one of these measured reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanupEvidence {
    ExplicitSelection {
        reason: String,
    },
    VerifiedDuplicate {
        kept_path: PathBuf,
        content_hash: String,
    },
    RegenerableTemporary {
        producer: String,
        proof: String,
    },
}

impl CleanupEvidence {
    fn validate(&self) -> Result<(), RecycleError> {
        match self {
            Self::ExplicitSelection { reason }
            | Self::RegenerableTemporary {
                producer: reason,
                proof: _,
            } if reason.trim().is_empty() => Err(RecycleError::Invalid(
                "cleanup evidence must explain why the target is disposable".into(),
            )),
            Self::VerifiedDuplicate {
                kept_path,
                content_hash,
            } if kept_path.as_os_str().is_empty() || content_hash.is_empty() => {
                Err(RecycleError::Invalid(
                    "duplicate cleanup evidence must name a kept path and hash".into(),
                ))
            }
            Self::RegenerableTemporary { proof, .. } if proof.trim().is_empty() => {
                Err(RecycleError::Invalid(
                    "temporary cleanup evidence must include measured proof".into(),
                ))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupCandidate {
    pub path: PathBuf,
    pub evidence: CleanupEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupTarget {
    pub target: RecycleTarget,
    pub evidence: CleanupEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPlan {
    pub id: String,
    pub provider_id: String,
    pub targets: Vec<CleanupTarget>,
}

impl CleanupPlan {
    pub fn digest(&self) -> PlanHash {
        digest_json(b"custodian-governed-cleanup-v1", self)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecycleOutcome {
    pub recycled: Vec<RecoveryReceipt>,
    pub skipped: Vec<(PathBuf, String)>,
    pub recovery_unverified: Vec<(PathBuf, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanupOutcome {
    pub recycled: Vec<RecoveryReceipt>,
    pub skipped: Vec<(PathBuf, String)>,
    pub recovery_unverified: Vec<(PathBuf, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecycleRestorePlan {
    pub id: String,
    pub receipt: RecoveryReceipt,
    pub destination: PathBuf,
    parent_identity: FileIdentity,
}

impl RecycleRestorePlan {
    pub fn digest(&self) -> PlanHash {
        digest_json(b"custodian-governed-recycle-restore-v1", self)
    }
}

/// A provider that intentionally refuses every operation. It is the safe
/// default until a supported native recovery provider is wired in.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedRecoveryProvider;

impl RecoveryProvider for UnsupportedRecoveryProvider {
    fn capability(&self) -> RecoveryCapability {
        RecoveryCapability {
            provider_id: "unsupported".into(),
            supports_recycle: false,
            supports_restore: false,
            verifies_recycle: false,
            verifies_restore: false,
        }
    }

    fn recycle(&self, _target: &RecycleTarget) -> Result<RecoveryReceipt, RecycleError> {
        Err(RecycleError::Unsupported(
            "no native recovery provider is qualified".into(),
        ))
    }

    fn verify_recycled(&self, _receipt: &RecoveryReceipt) -> Result<(), RecycleError> {
        Err(RecycleError::Unsupported(
            "recycle verification is unavailable".into(),
        ))
    }

    fn restore(&self, _receipt: &RecoveryReceipt, _destination: &Path) -> Result<(), RecycleError> {
        Err(RecycleError::Unsupported("restore is unavailable".into()))
    }

    fn verify_restored(
        &self,
        _receipt: &RecoveryReceipt,
        _destination: &Path,
    ) -> Result<(), RecycleError> {
        Err(RecycleError::Unsupported(
            "restore verification is unavailable".into(),
        ))
    }
}

/// Governs both explicit recycle and measured cleanup for one native provider.
/// The two plan types and digest domains stay distinct, so copy/backup
/// authority cannot be reused accidentally.
#[derive(Debug, Clone)]
pub struct GovernedRecycle<P> {
    provider: P,
}

impl<P: RecoveryProvider> GovernedRecycle<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn capability(&self) -> RecoveryCapability {
        self.provider.capability()
    }

    pub fn prepare_recycle(
        &self,
        paths: &[PathBuf],
        allowed: &dyn Fn(&Path) -> bool,
    ) -> Result<RecyclePlan, RecycleError> {
        self.require_supported()?;
        let targets = capture_targets(paths, allowed)?;
        Ok(RecyclePlan {
            id: uuid::Uuid::new_v4().to_string(),
            provider_id: self.capability().provider_id,
            targets,
        })
    }

    pub fn recycle_approved(
        &self,
        plan: &RecyclePlan,
        approval: &Approval,
        verifier: &Verifier,
        now_unix: u64,
    ) -> Result<RecycleOutcome, RecycleError> {
        self.require_supported()?;
        verifier.verify(&plan.digest(), approval, now_unix)?;
        if plan.provider_id != self.capability().provider_id {
            return Err(RecycleError::Unsupported(
                "the selected provider changed since approval".into(),
            ));
        }
        let mut outcome = RecycleOutcome::default();
        for target in &plan.targets {
            if let Err(reason) = recheck_target(target) {
                outcome
                    .skipped
                    .push((target.path.clone(), reason.to_string()));
                continue;
            }
            let receipt = match self.provider.recycle(target) {
                Ok(receipt) => receipt,
                Err(RecycleError::RecoveryUnverified(reason)) => {
                    outcome
                        .recovery_unverified
                        .push((target.path.clone(), reason));
                    continue;
                }
                Err(error) => return Err(error),
            };
            if receipt.provider_id != plan.provider_id
                || receipt.original_path != target.path
                || receipt.identity != target.identity
                || receipt.size != target.size
                || receipt.hash != target.hash
            {
                outcome.recovery_unverified.push((
                    target.path.clone(),
                    "provider returned a receipt for different data".into(),
                ));
                continue;
            }
            match self.provider.verify_recycled(&receipt) {
                Ok(()) => outcome.recycled.push(receipt),
                Err(error) => outcome
                    .recovery_unverified
                    .push((target.path.clone(), error.to_string())),
            }
        }
        Ok(outcome)
    }

    /// Build a cleanup plan only from measured evidence. There is no API that
    /// accepts age/size alone as a permission to recycle.
    pub fn prepare_cleanup(
        &self,
        candidates: &[CleanupCandidate],
        allowed: &dyn Fn(&Path) -> bool,
    ) -> Result<CleanupPlan, RecycleError> {
        self.require_supported()?;
        if candidates.is_empty() || candidates.len() > MAX_TARGETS {
            return Err(RecycleError::Invalid(format!(
                "cleanup must contain between 1 and {MAX_TARGETS} targets"
            )));
        }
        let mut targets = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            candidate.evidence.validate()?;
            targets.push(CleanupTarget {
                target: capture_target(&candidate.path, allowed)?,
                evidence: candidate.evidence.clone(),
            });
        }
        targets.sort_by(|left, right| left.target.path.cmp(&right.target.path));
        Ok(CleanupPlan {
            id: uuid::Uuid::new_v4().to_string(),
            provider_id: self.capability().provider_id,
            targets,
        })
    }

    pub fn cleanup_approved(
        &self,
        plan: &CleanupPlan,
        approval: &Approval,
        verifier: &Verifier,
        now_unix: u64,
    ) -> Result<CleanupOutcome, RecycleError> {
        self.require_supported()?;
        verifier.verify(&plan.digest(), approval, now_unix)?;
        if plan.provider_id != self.capability().provider_id {
            return Err(RecycleError::Unsupported(
                "the selected provider changed since approval".into(),
            ));
        }
        let mut outcome = CleanupOutcome::default();
        for cleanup in &plan.targets {
            let target = &cleanup.target;
            if let Err(reason) = recheck_target(target) {
                outcome
                    .skipped
                    .push((target.path.clone(), reason.to_string()));
                continue;
            }
            let receipt = match self.provider.recycle(target) {
                Ok(receipt) => receipt,
                Err(RecycleError::RecoveryUnverified(reason)) => {
                    outcome
                        .recovery_unverified
                        .push((target.path.clone(), reason));
                    continue;
                }
                Err(error) => return Err(error),
            };
            if receipt.provider_id != plan.provider_id
                || receipt.original_path != target.path
                || receipt.identity != target.identity
                || receipt.size != target.size
                || receipt.hash != target.hash
            {
                outcome.recovery_unverified.push((
                    target.path.clone(),
                    "provider returned a receipt for different data".into(),
                ));
                continue;
            }
            match self.provider.verify_recycled(&receipt) {
                Ok(()) => outcome.recycled.push(receipt),
                Err(error) => outcome
                    .recovery_unverified
                    .push((target.path.clone(), error.to_string())),
            }
        }
        Ok(outcome)
    }

    pub fn prepare_restore(
        &self,
        receipt: &RecoveryReceipt,
        destination: &Path,
    ) -> Result<RecycleRestorePlan, RecycleError> {
        self.require_supported()?;
        if receipt.provider_id != self.capability().provider_id {
            return Err(RecycleError::Unsupported(
                "receipt belongs to another provider".into(),
            ));
        }
        self.provider.verify_recycled(receipt)?;
        let destination = clean_destination(destination)?;
        if destination.exists() {
            return Err(RecycleError::RestoreDestinationChanged);
        }
        let parent = destination
            .parent()
            .ok_or_else(|| RecycleError::Invalid("restore destination needs a parent".into()))?;
        let parent = canonical_dir(parent)?;
        Ok(RecycleRestorePlan {
            id: uuid::Uuid::new_v4().to_string(),
            receipt: receipt.clone(),
            destination,
            parent_identity: file_identity(&parent)?,
        })
    }

    pub fn restore_approved(
        &self,
        plan: &RecycleRestorePlan,
        approval: &Approval,
        verifier: &Verifier,
        now_unix: u64,
    ) -> Result<(), RecycleError> {
        self.require_supported()?;
        verifier.verify(&plan.digest(), approval, now_unix)?;
        if plan.receipt.provider_id != self.capability().provider_id {
            return Err(RecycleError::Unsupported(
                "receipt belongs to another provider".into(),
            ));
        }
        let parent = plan
            .destination
            .parent()
            .ok_or_else(|| RecycleError::Invalid("restore destination needs a parent".into()))?;
        if file_identity(&canonical_dir(parent)?)? != plan.parent_identity
            || plan.destination.exists()
        {
            return Err(RecycleError::RestoreDestinationChanged);
        }
        self.provider.restore(&plan.receipt, &plan.destination)?;
        self.provider
            .verify_restored(&plan.receipt, &plan.destination)
    }

    fn require_supported(&self) -> Result<(), RecycleError> {
        let capability = self.capability();
        if capability.supported() {
            Ok(())
        } else {
            Err(RecycleError::Unsupported(format!(
                "provider {} does not offer verified recycle and restore",
                capability.provider_id
            )))
        }
    }
}

fn capture_targets(
    paths: &[PathBuf],
    allowed: &dyn Fn(&Path) -> bool,
) -> Result<Vec<RecycleTarget>, RecycleError> {
    if paths.is_empty() || paths.len() > MAX_TARGETS {
        return Err(RecycleError::Invalid(format!(
            "recycle must contain between 1 and {MAX_TARGETS} targets"
        )));
    }
    let mut targets = paths
        .iter()
        .map(|path| capture_target(path, allowed))
        .collect::<Result<Vec<_>, _>>()?;
    targets.sort_by(|left, right| left.path.cmp(&right.path));
    if targets.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(RecycleError::Invalid(
            "recycle targets must be unique".into(),
        ));
    }
    Ok(targets)
}

fn capture_target(
    path: &Path,
    allowed: &dyn Fn(&Path) -> bool,
) -> Result<RecycleTarget, RecycleError> {
    let path = canonical_file(path)?;
    if !allowed(&path) {
        return Err(RecycleError::Invalid(
            "target is outside the native recycle authority scope".into(),
        ));
    }
    let metadata = fs::symlink_metadata(&path)?;
    Ok(RecycleTarget {
        path: path.clone(),
        identity: file_identity(&path)?,
        size: metadata.len(),
        modified_nanos: modified_nanos(&metadata),
        hash: regular_hash(&path)?,
    })
}

fn recheck_target(target: &RecycleTarget) -> Result<(), RecycleError> {
    let path = canonical_file(&target.path)?;
    let metadata = fs::symlink_metadata(&path)?;
    if file_identity(&path)? != target.identity
        || metadata.len() != target.size
        || modified_nanos(&metadata) != target.modified_nanos
        || regular_hash(&path)? != target.hash
    {
        return Err(RecycleError::TargetChanged);
    }
    Ok(())
}

fn canonical_file(path: &Path) -> Result<PathBuf, RecycleError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(RecycleError::Invalid(
            "recycle targets must be absolute, normalized paths".into(),
        ));
    }
    let path = fs::canonicalize(path)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || is_reparse_point(&metadata) || !is_local_volume(&path) {
        return Err(RecycleError::Invalid(
            "only ordinary local files are supported by governed recycle".into(),
        ));
    }
    Ok(path)
}

fn canonical_dir(path: &Path) -> Result<PathBuf, RecycleError> {
    let path = fs::canonicalize(path)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) || !is_local_volume(&path) {
        return Err(RecycleError::Invalid(
            "expected an ordinary local directory".into(),
        ));
    }
    Ok(path)
}

fn clean_destination(path: &Path) -> Result<PathBuf, RecycleError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        || !is_local_volume(path)
    {
        return Err(RecycleError::Invalid(
            "restore destinations must be absolute normalized local paths".into(),
        ));
    }
    Ok(path.to_path_buf())
}

fn modified_nanos(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

fn regular_hash(path: &Path) -> Result<String, RecycleError> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn digest_json<T: Serialize>(domain: &[u8], value: &T) -> PlanHash {
    let bytes = serde_json::to_vec(value).expect("recycle plan serialization is infallible");
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(&bytes);
    hasher.finalize().into()
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct FakeProvider {
        trash: PathBuf,
        verify_recycle: bool,
        restored: Mutex<Vec<PathBuf>>,
    }

    impl RecoveryProvider for FakeProvider {
        fn capability(&self) -> RecoveryCapability {
            RecoveryCapability {
                provider_id: "test-provider".into(),
                supports_recycle: true,
                supports_restore: true,
                verifies_recycle: true,
                verifies_restore: true,
            }
        }

        fn recycle(&self, target: &RecycleTarget) -> Result<RecoveryReceipt, RecycleError> {
            fs::create_dir_all(&self.trash)?;
            let token = uuid::Uuid::new_v4().to_string();
            fs::rename(&target.path, self.trash.join(&token))?;
            Ok(RecoveryReceipt {
                provider_id: "test-provider".into(),
                token,
                original_path: target.path.clone(),
                identity: target.identity,
                size: target.size,
                hash: target.hash.clone(),
            })
        }

        fn verify_recycled(&self, receipt: &RecoveryReceipt) -> Result<(), RecycleError> {
            if !self.verify_recycle {
                return Err(RecycleError::RecoveryUnverified(
                    "test provider withheld recovery proof".into(),
                ));
            }
            let path = self.trash.join(&receipt.token);
            if receipt.original_path.exists() || regular_hash(&path)? != receipt.hash {
                return Err(RecycleError::RecoveryUnverified(
                    "recovery object does not match receipt".into(),
                ));
            }
            Ok(())
        }

        fn restore(
            &self,
            receipt: &RecoveryReceipt,
            destination: &Path,
        ) -> Result<(), RecycleError> {
            if destination.exists() {
                return Err(RecycleError::RestoreDestinationChanged);
            }
            fs::rename(self.trash.join(&receipt.token), destination)?;
            self.restored
                .lock()
                .unwrap()
                .push(destination.to_path_buf());
            Ok(())
        }

        fn verify_restored(
            &self,
            receipt: &RecoveryReceipt,
            destination: &Path,
        ) -> Result<(), RecycleError> {
            if !destination.is_file() || regular_hash(destination)? != receipt.hash {
                return Err(RecycleError::RecoveryUnverified(
                    "restored bytes do not match receipt".into(),
                ));
            }
            Ok(())
        }
    }

    fn approve(digest: PlanHash) -> (Approval, Verifier, u64) {
        let key = custodian_approval::ApprovalKey::generate();
        let now = now_unix();
        (
            key.sign(&digest, now, 300),
            Verifier::new(key.verifying_key()),
            now,
        )
    }

    #[test]
    fn unsupported_recovery_refuses_without_unlinking() {
        let fixture = tempfile::tempdir().unwrap();
        let file = fixture.path().join("important.txt");
        fs::write(&file, b"keep me").unwrap();
        let service = GovernedRecycle::new(UnsupportedRecoveryProvider);
        let result = service.prepare_recycle(&[file.clone()], &|_| true);
        assert!(matches!(result, Err(RecycleError::Unsupported(_))));
        assert_eq!(fs::read(file).unwrap(), b"keep me");
    }

    #[test]
    fn wrong_authority_refuses_before_provider_mutation() {
        let fixture = tempfile::tempdir().unwrap();
        let file = fixture.path().join("important.txt");
        fs::write(&file, b"keep me").unwrap();
        let provider = FakeProvider {
            trash: fixture.path().join("trash"),
            verify_recycle: true,
            restored: Mutex::new(Vec::new()),
        };
        let service = GovernedRecycle::new(provider);
        let plan = service.prepare_recycle(&[file.clone()], &|_| true).unwrap();
        let wrong_key = custodian_approval::ApprovalKey::generate();
        let now = now_unix();
        let approval = wrong_key.sign(&[3u8; 32], now, 300);
        let verifier = Verifier::new(wrong_key.verifying_key());
        assert!(matches!(
            service.recycle_approved(&plan, &approval, &verifier, now),
            Err(RecycleError::NotAuthorized(ApprovalError::Auth))
        ));
        assert!(file.exists());
    }

    #[test]
    fn verified_recycle_can_be_restored_without_permanent_delete() {
        let fixture = tempfile::tempdir().unwrap();
        let file = fixture.path().join("important.txt");
        fs::write(&file, b"recoverable").unwrap();
        let service = GovernedRecycle::new(FakeProvider {
            trash: fixture.path().join("trash"),
            verify_recycle: true,
            restored: Mutex::new(Vec::new()),
        });
        let plan = service.prepare_recycle(&[file.clone()], &|_| true).unwrap();
        let (approval, verifier, now) = approve(plan.digest());
        let outcome = service
            .recycle_approved(&plan, &approval, &verifier, now)
            .unwrap();
        assert_eq!(outcome.recycled.len(), 1);
        let receipt = &outcome.recycled[0];
        assert!(!file.exists());
        let destination = fixture.path().join("restored.txt");
        let restore = service.prepare_restore(receipt, &destination).unwrap();
        let (approval, verifier, now) = approve(restore.digest());
        service
            .restore_approved(&restore, &approval, &verifier, now)
            .unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"recoverable");
    }

    #[test]
    fn recovery_unverified_is_reported_and_never_claimed_as_recycled() {
        let fixture = tempfile::tempdir().unwrap();
        let file = fixture.path().join("important.txt");
        fs::write(&file, b"recoverable").unwrap();
        let service = GovernedRecycle::new(FakeProvider {
            trash: fixture.path().join("trash"),
            verify_recycle: false,
            restored: Mutex::new(Vec::new()),
        });
        let plan = service.prepare_recycle(&[file.clone()], &|_| true).unwrap();
        let (approval, verifier, now) = approve(plan.digest());
        let outcome = service
            .recycle_approved(&plan, &approval, &verifier, now)
            .unwrap();
        assert!(outcome.recycled.is_empty());
        assert_eq!(outcome.recovery_unverified.len(), 1);
        // The provider's recoverable object is not silently unlinked by this
        // module, even though the provider refused to prove it.
        assert!(!file.exists());
        assert_eq!(
            fs::read_dir(fixture.path().join("trash")).unwrap().count(),
            1
        );
    }

    #[test]
    fn cleanup_requires_explainable_evidence_and_rechecks_newer_edits() {
        let fixture = tempfile::tempdir().unwrap();
        let file = fixture.path().join("temporary.tmp");
        fs::write(&file, b"old").unwrap();
        let service = GovernedRecycle::new(FakeProvider {
            trash: fixture.path().join("trash"),
            verify_recycle: true,
            restored: Mutex::new(Vec::new()),
        });
        let invalid = CleanupCandidate {
            path: file.clone(),
            evidence: CleanupEvidence::ExplicitSelection { reason: " ".into() },
        };
        assert!(matches!(
            service.prepare_cleanup(&[invalid], &|_| true),
            Err(RecycleError::Invalid(_))
        ));
        let candidate = CleanupCandidate {
            path: file.clone(),
            evidence: CleanupEvidence::RegenerableTemporary {
                producer: "fixture".into(),
                proof: "measured fixture output".into(),
            },
        };
        let plan = service.prepare_cleanup(&[candidate], &|_| true).unwrap();
        fs::write(&file, b"newer edit").unwrap();
        let (approval, verifier, now) = approve(plan.digest());
        let outcome = service
            .cleanup_approved(&plan, &approval, &verifier, now)
            .unwrap();
        assert!(outcome.recycled.is_empty());
        assert_eq!(outcome.skipped.len(), 1);
        assert_eq!(fs::read(file).unwrap(), b"newer edit");
    }
}
