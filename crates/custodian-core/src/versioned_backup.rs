//! Versioned local backups with an integrity manifest and an independent
//! restore verifier.
//!
//! This module deliberately has no language/content understanding and no
//! delete primitive. A backup plan contains the exact source snapshot that was
//! selected by a trusted caller. The plan is approved as a whole, copied into
//! a new version directory, and only published as `Verified` after a second
//! read of both the sources and the copied tree. A version that is interrupted
//! or later damaged remains unusable.
//!
//! The module is intentionally not exported from `lib.rs` in this increment.
//! The integration point must bind `BackupStore::prepare_backup` to the native
//! privacy policy and issue a consequence-specific approval before exposing
//! these APIs.
//!
//! The manifest preserves exact relative paths, ordinary-file bytes, sizes and
//! content digests. Source modification time and identity are preconditions,
//! not restored metadata; permissions, ACLs, alternate streams and other
//! platform metadata are intentionally unsupported until separately
//! qualified. The local store is not encrypted by this module and does not
//! provide ransomware-proof or off-machine protection; deployment must apply
//! OS access controls and an independent copy policy.

use custodian_approval::{Approval, ApprovalError, PlanHash, Verifier};
use custodian_winsec::fsops::{file_identity, is_local_volume, FileIdentity};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const OWNER_FILE: &str = "owner.json";
const VERSIONS_DIR: &str = "versions";
const FILES_DIR: &str = "files";
const MANIFEST_FILE: &str = "manifest.json";
const MANIFEST_HASH_FILE: &str = "manifest.blake3";
const STATE_FILE: &str = "state.json";
const MAX_SOURCES: usize = 256;
const MAX_ENTRIES: usize = 100_000;

/// A backup failure is explicit so callers cannot mistake an incomplete copy
/// for a successful backup or restore.
#[derive(Debug, Error)]
pub enum BackupError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("manifest error: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("backup authority refused the plan: {0}")]
    NotAuthorized(#[from] ApprovalError),
    #[error("invalid backup request: {0}")]
    Invalid(String),
    #[error("source changed while the backup was being made: {0}")]
    SourceChanged(String),
    #[error("backup destination changed since the plan was prepared")]
    DestinationChanged,
    #[error("backup version {0} was not found")]
    MissingVersion(String),
    #[error("backup version is incomplete and cannot be used")]
    Incomplete,
    #[error("backup integrity verification failed: {0}")]
    Integrity(String),
    #[error("backup version is not owned by this store")]
    NotOwned,
    #[error("restore destination changed since the plan was prepared")]
    RestoreDestinationChanged,
    #[error("restore destination is not a new location")]
    DestinationExists,
    #[error("restore was interrupted; its staging directory was retained")]
    Interrupted,
}

/// A version is either still being written or has passed independent
/// verification. There is no `Failed` state that could be mistaken for a
/// usable recovery point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionState {
    Incomplete,
    Verified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EntryKind {
    File,
    Directory,
}

/// Retention never counts incomplete or corrupted versions as healthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// The number of independently verified versions that must remain.
    pub keep_at_least: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self { keep_at_least: 1 }
    }
}

impl RetentionPolicy {
    fn validate(self) -> Result<Self, BackupError> {
        if self.keep_at_least == 0 {
            return Err(BackupError::Invalid(
                "retention must preserve at least one verified version".into(),
            ));
        }
        Ok(self)
    }
}

/// The exact source item captured during planning. Directory entries are
/// included so the source tree cannot silently gain or lose a child between
/// planning and publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEntry {
    pub source: PathBuf,
    pub relative_path: PathBuf,
    pub kind: String,
    pub size: u64,
    pub modified_nanos: u128,
    pub identity: FileIdentity,
    pub hash: Option<String>,
}

impl SourceEntry {
    fn entry_kind(&self) -> EntryKind {
        if self.kind == "directory" {
            EntryKind::Directory
        } else {
            EntryKind::File
        }
    }
}

/// An exact, approval-bound selection and destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupPlan {
    pub id: String,
    pub destination: PathBuf,
    pub destination_identity: FileIdentity,
    pub source_roots: Vec<PathBuf>,
    pub entries: Vec<SourceEntry>,
    pub retention: RetentionPolicy,
}

impl BackupPlan {
    /// The digest signed by the native backup authority. It covers paths,
    /// identities, bytes, hashes, destination and retention policy.
    pub fn digest(&self) -> PlanHash {
        digest_json(b"custodian-versioned-backup-v1", self)
    }

    pub fn total_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.size).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ManifestEntry {
    relative_path: PathBuf,
    kind: EntryKind,
    size: u64,
    hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Manifest {
    schema: u32,
    version_id: String,
    owner_id: String,
    created_unix: u64,
    source_roots: Vec<PathBuf>,
    entries: Vec<ManifestEntry>,
    tree_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OwnerRecord {
    owner_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StateRecord {
    version_id: String,
    owner_id: String,
    created_unix: u64,
    state: VersionState,
}

/// A version summary returned by listing or creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSummary {
    pub id: String,
    pub state: VersionState,
    pub created_unix: u64,
    pub entries: usize,
    pub bytes: u64,
}

/// A verified version. The manifest is private so callers must use this
/// module's verification path instead of manufacturing a trusted manifest.
#[derive(Debug, Clone)]
pub struct VerifiedVersion {
    pub id: String,
    pub path: PathBuf,
    manifest: Manifest,
    manifest_digest: String,
}

impl VerifiedVersion {
    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    pub fn entries(&self) -> usize {
        self.manifest.entries.len()
    }

    pub fn bytes(&self) -> u64 {
        self.manifest.entries.iter().map(|entry| entry.size).sum()
    }
}

/// Restore to a new location by default. `PreserveExisting` is explicit and
/// never overwrites a destination file that differs from the verified backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreMode {
    NewLocation,
    PreserveExisting,
}

impl Default for RestoreMode {
    fn default() -> Self {
        Self::NewLocation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestorePlan {
    pub id: String,
    pub version_id: String,
    pub destination: PathBuf,
    pub mode: RestoreMode,
    destination_parent_identity: FileIdentity,
    destination_identity: Option<FileIdentity>,
    manifest_digest: String,
}

impl RestorePlan {
    pub fn digest(&self) -> PlanHash {
        digest_json(b"custodian-versioned-restore-v1", self)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestoreOutcome {
    pub version_id: String,
    pub copied: usize,
    pub already_present: usize,
    pub preserved_newer: Vec<PathBuf>,
    pub verified: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeVerification {
    pub exact: bool,
    pub missing: Vec<PathBuf>,
    pub changed: Vec<PathBuf>,
    pub extra: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPlan {
    pub id: String,
    pub keep_at_least: usize,
    pub candidates: Vec<String>,
    owner_id: String,
}

impl RetentionPlan {
    pub fn digest(&self) -> PlanHash {
        digest_json(b"custodian-versioned-retention-v1", self)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetentionOutcome {
    pub removed: Vec<String>,
    pub preserved: Vec<String>,
}

/// The local version store. Its owner marker prevents a retention plan from
/// deleting another store's versions even if they share a parent directory.
#[derive(Debug, Clone)]
pub struct BackupStore {
    root: PathBuf,
    owner_id: String,
}

impl BackupStore {
    /// Open or initialize a local backup store. The root itself must be a
    /// non-reparse local directory; no source is read by this constructor.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, BackupError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        let root = canonical_dir(&root)?;
        let owner_path = root.join(OWNER_FILE);
        let owner_id = match fs::read(&owner_path) {
            Ok(bytes) => {
                let owner: OwnerRecord = serde_json::from_slice(&bytes)?;
                if uuid::Uuid::parse_str(&owner.owner_id).is_err() {
                    return Err(BackupError::Invalid("invalid backup owner marker".into()));
                }
                owner.owner_id
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let owner_id = uuid::Uuid::new_v4().to_string();
                write_json_new(
                    &owner_path,
                    &OwnerRecord {
                        owner_id: owner_id.clone(),
                    },
                )?;
                owner_id
            }
            Err(error) => return Err(error.into()),
        };
        fs::create_dir_all(root.join(VERSIONS_DIR))?;
        Ok(Self { root, owner_id })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    /// Capture the exact selected file/directory tree before any copy starts.
    /// A caller integrating this module must perform the native privacy check
    /// for each root and destination before calling this method.
    pub fn prepare_backup(
        &self,
        sources: &[PathBuf],
        retention: RetentionPolicy,
    ) -> Result<BackupPlan, BackupError> {
        if sources.is_empty() || sources.len() > MAX_SOURCES {
            return Err(BackupError::Invalid(format!(
                "select between 1 and {MAX_SOURCES} sources"
            )));
        }
        let retention = retention.validate()?;
        let destination_identity = file_identity(&self.root)?;
        let source_roots = sources
            .iter()
            .map(|source| canonical_source(source, &self.root))
            .collect::<Result<Vec<_>, _>>()?;
        if source_roots
            .iter()
            .any(|source| source == &self.root || source.starts_with(&self.root))
        {
            return Err(BackupError::Invalid(
                "a backup source cannot include its own version store".into(),
            ));
        }
        let mut entries = Vec::new();
        let mut occupied = BTreeSet::new();
        for source in &source_roots {
            let name = source
                .file_name()
                .ok_or_else(|| BackupError::Invalid("source has no name".into()))?;
            collect_source(source, Path::new(name), &mut entries, &mut occupied)?;
        }
        if entries.is_empty() || entries.len() > MAX_ENTRIES {
            return Err(BackupError::Invalid(format!(
                "selection must contain between 1 and {MAX_ENTRIES} entries"
            )));
        }
        entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(BackupPlan {
            id: uuid::Uuid::new_v4().to_string(),
            destination: self.root.clone(),
            destination_identity,
            source_roots,
            entries,
            retention,
        })
    }

    /// Execute an approved plan. `cancel` is checked before each entry and
    /// the version remains visibly incomplete if it is set or if a source
    /// changes. A new approval is required to retry a new plan.
    pub fn create_version(
        &self,
        plan: &BackupPlan,
        approval: &Approval,
        verifier: &Verifier,
        now_unix: u64,
        cancel: &AtomicBool,
    ) -> Result<VersionSummary, BackupError> {
        verifier.verify(&plan.digest(), approval, now_unix)?;
        self.validate_plan_destination(plan)?;
        verify_source_snapshot(plan)?;

        let version_dir = self.root.join(VERSIONS_DIR).join(&plan.id);
        fs::create_dir(&version_dir)?;
        let state = StateRecord {
            version_id: plan.id.clone(),
            owner_id: self.owner_id.clone(),
            created_unix: now_unix,
            state: VersionState::Incomplete,
        };
        write_json_atomic(&version_dir.join(STATE_FILE), &state)?;
        let files_dir = version_dir.join(FILES_DIR);
        if let Err(error) = self.copy_entries(plan, &files_dir, cancel) {
            return Err(error);
        }
        if let Err(error) = verify_source_snapshot(plan) {
            return Err(error);
        }

        let entries = plan
            .entries
            .iter()
            .map(|entry| ManifestEntry {
                relative_path: entry.relative_path.clone(),
                kind: entry.entry_kind(),
                size: entry.size,
                hash: entry.hash.clone(),
            })
            .collect::<Vec<_>>();
        let manifest = Manifest {
            schema: 1,
            version_id: plan.id.clone(),
            owner_id: self.owner_id.clone(),
            created_unix: now_unix,
            source_roots: plan.source_roots.clone(),
            tree_digest: tree_digest(&entries),
            entries,
        };
        let manifest_path = version_dir.join(MANIFEST_FILE);
        write_json_atomic(&manifest_path, &manifest)?;
        let manifest_bytes = fs::read(&manifest_path)?;
        write_text_atomic(
            &version_dir.join(MANIFEST_HASH_FILE),
            &hex_digest(&manifest_bytes),
        )?;

        // This is a second verifier, not a trust in the copy loop's counters.
        verify_version_files(&version_dir, &self.owner_id, VersionState::Incomplete)?;
        let verified_state = StateRecord {
            state: VersionState::Verified,
            ..state
        };
        write_json_atomic(&version_dir.join(STATE_FILE), &verified_state)?;
        let verified = self.verify_version(&plan.id)?;
        Ok(VersionSummary {
            id: verified.id.clone(),
            state: VersionState::Verified,
            created_unix: now_unix,
            entries: verified.entries(),
            bytes: verified.bytes(),
        })
    }

    /// Return all version directories, including incomplete versions. A
    /// caller must use `verify_version` before treating a summary as usable.
    pub fn list_versions(&self) -> Result<Vec<VersionSummary>, BackupError> {
        let mut summaries = Vec::new();
        for entry in fs::read_dir(self.root.join(VERSIONS_DIR))? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.is_dir() || is_reparse_point(&metadata) {
                continue;
            }
            let state: StateRecord = match read_json(&path.join(STATE_FILE)) {
                Ok(state) => state,
                Err(_) => continue,
            };
            if state.owner_id != self.owner_id || state.version_id != path_name(&path)? {
                continue;
            }
            let (entries, bytes) = match read_json::<Manifest>(&path.join(MANIFEST_FILE)) {
                Ok(manifest) => (
                    manifest.entries.len(),
                    manifest.entries.iter().map(|entry| entry.size).sum(),
                ),
                Err(_) => (0, 0),
            };
            summaries.push(VersionSummary {
                id: state.version_id,
                state: state.state,
                created_unix: state.created_unix,
                entries,
                bytes,
            });
        }
        summaries.sort_by(|left, right| {
            left.created_unix
                .cmp(&right.created_unix)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(summaries)
    }

    /// Independently verify state, manifest bytes, every payload and the exact
    /// payload tree. Incomplete/corrupt versions are never returned as healthy.
    pub fn verify_version(&self, version_id: &str) -> Result<VerifiedVersion, BackupError> {
        let version_dir = self.version_dir(version_id)?;
        let state: StateRecord = read_json(&version_dir.join(STATE_FILE))?;
        if state.owner_id != self.owner_id {
            return Err(BackupError::NotOwned);
        }
        if state.state != VersionState::Verified {
            return Err(BackupError::Incomplete);
        }
        if state.version_id != version_id {
            return Err(BackupError::Integrity(
                "state identity does not match path".into(),
            ));
        }
        let (manifest, manifest_digest) = read_manifest(&version_dir)?;
        if manifest.owner_id != self.owner_id || manifest.version_id != version_id {
            return Err(BackupError::NotOwned);
        }
        verify_version_files(&version_dir, &self.owner_id, VersionState::Verified)?;
        Ok(VerifiedVersion {
            id: version_id.to_string(),
            path: version_dir,
            manifest,
            manifest_digest,
        })
    }

    /// Capture a restore operation against one verified version. The default
    /// mode requires a destination that does not yet exist.
    pub fn prepare_restore(
        &self,
        version_id: &str,
        destination: &Path,
        mode: RestoreMode,
    ) -> Result<RestorePlan, BackupError> {
        let version = self.verify_version(version_id)?;
        let destination = clean_destination(destination)?;
        if destination == self.root || destination.starts_with(&self.root) {
            return Err(BackupError::Invalid(
                "restore destination cannot be inside the backup store".into(),
            ));
        }
        let parent = destination
            .parent()
            .ok_or_else(|| BackupError::Invalid("restore destination needs a parent".into()))?;
        let parent = canonical_dir(parent)?;
        let parent_identity = file_identity(&parent)?;
        let destination_identity = match mode {
            RestoreMode::NewLocation if destination.exists() => {
                return Err(BackupError::DestinationExists)
            }
            RestoreMode::NewLocation => None,
            RestoreMode::PreserveExisting => {
                if destination.exists() {
                    Some(directory_identity(&destination)?)
                } else {
                    None
                }
            }
        };
        Ok(RestorePlan {
            id: uuid::Uuid::new_v4().to_string(),
            version_id: version.id,
            destination,
            mode,
            destination_parent_identity: parent_identity,
            destination_identity,
            manifest_digest: version.manifest_digest,
        })
    }

    /// Explicit convenience API for the safe default: restore to a new
    /// destination without overwriting anything that already exists there.
    pub fn prepare_restore_to_new_location(
        &self,
        version_id: &str,
        destination: &Path,
    ) -> Result<RestorePlan, BackupError> {
        self.prepare_restore(version_id, destination, RestoreMode::NewLocation)
    }

    /// Restore with a separate approval. New-location restore stages outside
    /// the final path and publishes only after an independent tree check.
    pub fn restore(
        &self,
        plan: &RestorePlan,
        approval: &Approval,
        verifier: &Verifier,
        now_unix: u64,
        cancel: &AtomicBool,
    ) -> Result<RestoreOutcome, BackupError> {
        verifier.verify(&plan.digest(), approval, now_unix)?;
        let version = self.verify_version(&plan.version_id)?;
        if version.manifest_digest != plan.manifest_digest {
            return Err(BackupError::Integrity(
                "the selected version changed since restore planning".into(),
            ));
        }
        validate_restore_destination(plan)?;
        match plan.mode {
            RestoreMode::NewLocation => self.restore_new_location(plan, &version, cancel),
            RestoreMode::PreserveExisting => {
                self.restore_preserving_existing(plan, &version, cancel)
            }
        }
    }

    /// Recheck a restored tree independently of the restore receipt.
    pub fn verify_restored(
        &self,
        version_id: &str,
        destination: &Path,
    ) -> Result<TreeVerification, BackupError> {
        let version = self.verify_version(version_id)?;
        Ok(compare_tree(destination, &version.manifest))
    }

    /// Prepare retention from the currently verified and owned versions. Bad
    /// and incomplete versions are not candidates and are never deleted.
    pub fn prepare_retention(&self, keep_at_least: usize) -> Result<RetentionPlan, BackupError> {
        let keep_at_least = RetentionPolicy { keep_at_least }.validate()?.keep_at_least;
        let mut healthy = self
            .list_versions()?
            .into_iter()
            .filter_map(|summary| self.verify_version(&summary.id).ok().map(|_| summary))
            .collect::<Vec<_>>();
        healthy.sort_by(|left, right| {
            left.created_unix
                .cmp(&right.created_unix)
                .then_with(|| left.id.cmp(&right.id))
        });
        let remove_count = healthy.len().saturating_sub(keep_at_least);
        let candidates = healthy
            .into_iter()
            .take(remove_count)
            .map(|summary| summary.id)
            .collect();
        Ok(RetentionPlan {
            id: uuid::Uuid::new_v4().to_string(),
            keep_at_least,
            candidates,
            owner_id: self.owner_id.clone(),
        })
    }

    /// Delete only exact, currently verified directories owned by this store.
    /// It will not remove the last healthy version, an incomplete version, a
    /// corrupted version, or a neighboring directory.
    pub fn retain_verified(
        &self,
        plan: &RetentionPlan,
        approval: &Approval,
        verifier: &Verifier,
        now_unix: u64,
    ) -> Result<RetentionOutcome, BackupError> {
        verifier.verify(&plan.digest(), approval, now_unix)?;
        if plan.owner_id != self.owner_id {
            return Err(BackupError::NotOwned);
        }
        let healthy_count = self
            .list_versions()?
            .iter()
            .filter(|summary| self.verify_version(&summary.id).is_ok())
            .count();
        if healthy_count <= plan.keep_at_least {
            return Ok(RetentionOutcome {
                preserved: plan.candidates.clone(),
                ..RetentionOutcome::default()
            });
        }
        let mut outcome = RetentionOutcome::default();
        let mut remaining = healthy_count;
        for id in &plan.candidates {
            if remaining <= plan.keep_at_least {
                outcome.preserved.push(id.clone());
                continue;
            }
            let version = match self.verify_version(id) {
                Ok(version) => version,
                Err(_) => {
                    outcome.preserved.push(id.clone());
                    continue;
                }
            };
            if version.manifest.owner_id != self.owner_id {
                outcome.preserved.push(id.clone());
                continue;
            }
            match fs::remove_dir_all(&version.path) {
                Ok(()) => {
                    remaining -= 1;
                    outcome.removed.push(id.clone());
                }
                Err(error) => {
                    // The directory was independently verified immediately
                    // before this operation, and its path is an exact
                    // backup-owned version path. Nothing beside it is touched.
                    return Err(error.into());
                }
            }
        }
        Ok(outcome)
    }

    fn validate_plan_destination(&self, plan: &BackupPlan) -> Result<(), BackupError> {
        if plan.destination != self.root || file_identity(&self.root)? != plan.destination_identity
        {
            return Err(BackupError::DestinationChanged);
        }
        Ok(())
    }

    fn copy_entries(
        &self,
        plan: &BackupPlan,
        files_dir: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), BackupError> {
        fs::create_dir_all(files_dir)?;
        for entry in &plan.entries {
            if cancel.load(Ordering::Acquire) {
                return Err(BackupError::Interrupted);
            }
            let target = safe_join(files_dir, &entry.relative_path)?;
            match entry.entry_kind() {
                EntryKind::Directory => fs::create_dir_all(target)?,
                EntryKind::File => {
                    let expected = entry
                        .hash
                        .as_deref()
                        .ok_or_else(|| BackupError::Integrity("file has no hash".into()))?;
                    copy_file_verified(&entry.source, &target, entry.size, expected)?;
                }
            }
        }
        Ok(())
    }

    fn restore_new_location(
        &self,
        plan: &RestorePlan,
        version: &VerifiedVersion,
        cancel: &AtomicBool,
    ) -> Result<RestoreOutcome, BackupError> {
        if plan.destination.exists() {
            return Err(BackupError::RestoreDestinationChanged);
        }
        let parent = plan
            .destination
            .parent()
            .ok_or_else(|| BackupError::Invalid("restore destination needs a parent".into()))?;
        let name = plan
            .destination
            .file_name()
            .ok_or_else(|| BackupError::Invalid("restore destination needs a name".into()))?
            .to_string_lossy();
        let staging = parent.join(format!(".{name}.custodian-restore-{}.incomplete", plan.id));
        fs::create_dir(&staging)?;
        // Keep the recovery marker beside, rather than inside, the tree being
        // verified, so it can never become an accidental restored file.
        let state_path = parent.join(format!(".{name}.custodian-restore-{}.state.json", plan.id));
        write_json_atomic(
            &state_path,
            &RestoreState {
                version_id: version.id.clone(),
                manifest_digest: version.manifest_digest.clone(),
                complete: false,
            },
        )?;
        let result = (|| {
            for entry in &version.manifest.entries {
                if cancel.load(Ordering::Acquire) {
                    return Err(BackupError::Interrupted);
                }
                let target = safe_join(&staging, &entry.relative_path)?;
                match entry.kind {
                    EntryKind::Directory => fs::create_dir_all(target)?,
                    EntryKind::File => {
                        let source =
                            safe_join(&version.path.join(FILES_DIR), &entry.relative_path)?;
                        copy_file_verified(
                            &source,
                            &target,
                            entry.size,
                            entry
                                .hash
                                .as_deref()
                                .ok_or_else(|| BackupError::Integrity("file has no hash".into()))?,
                        )?;
                    }
                }
            }
            let checked = compare_tree(&staging, &version.manifest);
            if !checked.exact {
                return Err(BackupError::Integrity(format_tree_error(&checked)));
            }
            write_json_atomic(
                &state_path,
                &RestoreState {
                    version_id: version.id.clone(),
                    manifest_digest: version.manifest_digest.clone(),
                    complete: true,
                },
            )?;
            fs::remove_file(&state_path)?;
            fs::rename(&staging, &plan.destination)
                .map_err(|_| BackupError::RestoreDestinationChanged)?;
            Ok(RestoreOutcome {
                version_id: version.id.clone(),
                copied: version
                    .manifest
                    .entries
                    .iter()
                    .filter(|entry| entry.kind == EntryKind::File)
                    .count(),
                already_present: 0,
                preserved_newer: Vec::new(),
                verified: true,
            })
        })();
        result
    }

    fn restore_preserving_existing(
        &self,
        plan: &RestorePlan,
        version: &VerifiedVersion,
        cancel: &AtomicBool,
    ) -> Result<RestoreOutcome, BackupError> {
        if !plan.destination.exists() {
            fs::create_dir(&plan.destination)?;
        }
        let mut outcome = RestoreOutcome {
            version_id: version.id.clone(),
            ..RestoreOutcome::default()
        };
        let mut blocked = Vec::new();
        for entry in &version.manifest.entries {
            if cancel.load(Ordering::Acquire) {
                return Err(BackupError::Interrupted);
            }
            if blocked
                .iter()
                .any(|prefix: &PathBuf| entry.relative_path.starts_with(prefix))
            {
                outcome.preserved_newer.push(entry.relative_path.clone());
                continue;
            }
            check_restore_path(&plan.destination, &entry.relative_path)?;
            let target = safe_join(&plan.destination, &entry.relative_path)?;
            match entry.kind {
                EntryKind::Directory => {
                    if target.exists() && !target.is_dir() {
                        blocked.push(entry.relative_path.clone());
                        outcome.preserved_newer.push(entry.relative_path.clone());
                    } else {
                        fs::create_dir_all(target)?;
                    }
                }
                EntryKind::File => {
                    let expected = entry
                        .hash
                        .as_deref()
                        .ok_or_else(|| BackupError::Integrity("file has no hash".into()))?;
                    if target.exists() {
                        if regular_hash(&target).ok().as_deref() == Some(expected)
                            && fs::metadata(&target)?.len() == entry.size
                        {
                            outcome.already_present += 1;
                        } else {
                            outcome.preserved_newer.push(entry.relative_path.clone());
                        }
                        continue;
                    }
                    let source = safe_join(&version.path.join(FILES_DIR), &entry.relative_path)?;
                    match copy_file_verified(&source, &target, entry.size, expected) {
                        Ok(()) => outcome.copied += 1,
                        Err(BackupError::Io(error))
                            if error.kind() == io::ErrorKind::AlreadyExists =>
                        {
                            outcome.preserved_newer.push(entry.relative_path.clone());
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
        }
        let checked = compare_tree(&plan.destination, &version.manifest);
        outcome.verified = checked.exact;
        Ok(outcome)
    }

    fn version_dir(&self, version_id: &str) -> Result<PathBuf, BackupError> {
        let parsed = uuid::Uuid::parse_str(version_id)
            .map_err(|_| BackupError::MissingVersion(version_id.to_string()))?;
        let path = self.root.join(VERSIONS_DIR).join(parsed.to_string());
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| BackupError::MissingVersion(version_id.to_string()))?;
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return Err(BackupError::MissingVersion(version_id.to_string()));
        }
        Ok(path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RestoreState {
    version_id: String,
    manifest_digest: String,
    complete: bool,
}

fn digest_json<T: Serialize>(domain: &[u8], value: &T) -> PlanHash {
    let bytes = serde_json::to_vec(value).expect("backup plan serialization is infallible");
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

fn canonical_dir(path: &Path) -> Result<PathBuf, BackupError> {
    let path = fs::canonicalize(path)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) || !is_local_volume(&path) {
        return Err(BackupError::Invalid(
            "backup locations must be ordinary local directories".into(),
        ));
    }
    Ok(path)
}

fn canonical_source(path: &Path, store_root: &Path) -> Result<PathBuf, BackupError> {
    let path = fs::canonicalize(path)?;
    if path == store_root || path.starts_with(store_root) {
        return Err(BackupError::Invalid(
            "a source cannot be inside the backup store".into(),
        ));
    }
    let metadata = fs::symlink_metadata(&path)?;
    if is_reparse_point(&metadata) || !is_local_volume(&path) {
        return Err(BackupError::Invalid(
            "backup sources must be ordinary local paths".into(),
        ));
    }
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(BackupError::Invalid(
            "backup sources must be files or directories".into(),
        ));
    }
    Ok(path)
}

fn clean_destination(path: &Path) -> Result<PathBuf, BackupError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(BackupError::Invalid(
            "restore destinations must be absolute, normalized paths".into(),
        ));
    }
    if !is_local_volume(path) {
        return Err(BackupError::Invalid(
            "restore destinations must be on a local volume".into(),
        ));
    }
    Ok(path.to_path_buf())
}

fn directory_identity(path: &Path) -> Result<FileIdentity, BackupError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(BackupError::Invalid(
            "expected an ordinary directory".into(),
        ));
    }
    Ok(file_identity(path)?)
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

fn collect_source(
    source: &Path,
    relative: &Path,
    entries: &mut Vec<SourceEntry>,
    occupied: &mut BTreeSet<PathBuf>,
) -> Result<(), BackupError> {
    ensure_relative(relative)?;
    let metadata = fs::symlink_metadata(source)?;
    if is_reparse_point(&metadata) {
        return Err(BackupError::Invalid(
            "reparse points and symlinks are unsupported in backups".into(),
        ));
    }
    let identity = file_identity(source)?;
    let (kind, hash) = if metadata.is_dir() {
        ("directory".to_string(), None)
    } else if metadata.is_file() {
        ("file".to_string(), Some(regular_hash(source)?))
    } else {
        return Err(BackupError::Invalid(
            "backup selection contains a non-file object".into(),
        ));
    };
    if !occupied.insert(relative.to_path_buf()) {
        return Err(BackupError::Invalid(format!(
            "selected sources collide at {}",
            relative.display()
        )));
    }
    entries.push(SourceEntry {
        source: source.to_path_buf(),
        relative_path: relative.to_path_buf(),
        kind: kind.clone(),
        size: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        modified_nanos: modified_nanos(&metadata),
        identity,
        hash,
    });
    if kind == "directory" {
        let mut children = fs::read_dir(source)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        children.sort();
        for child in children {
            let child_name = child
                .file_name()
                .ok_or_else(|| BackupError::Invalid("source child has no name".into()))?;
            collect_source(&child, &relative.join(child_name), entries, occupied)?;
        }
    }
    Ok(())
}

fn verify_source_snapshot(plan: &BackupPlan) -> Result<(), BackupError> {
    let mut current = Vec::new();
    let mut occupied = BTreeSet::new();
    for source in &plan.source_roots {
        let name = source
            .file_name()
            .ok_or_else(|| BackupError::SourceChanged("source has no name".into()))?;
        collect_source(source, Path::new(name), &mut current, &mut occupied)
            .map_err(|error| BackupError::SourceChanged(error.to_string()))?;
    }
    current.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if current != plan.entries {
        return Err(BackupError::SourceChanged(
            "the selected source tree no longer matches the planned snapshot".into(),
        ));
    }
    Ok(())
}

fn ensure_relative(path: &Path) -> Result<(), BackupError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(BackupError::Invalid(
            "manifest paths must be relative and cannot traverse".into(),
        ));
    }
    Ok(())
}

fn safe_join(root: &Path, relative: &Path) -> Result<PathBuf, BackupError> {
    ensure_relative(relative)?;
    Ok(root.join(relative))
}

fn check_restore_path(root: &Path, relative: &Path) -> Result<(), BackupError> {
    ensure_relative(relative)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(BackupError::Invalid(
                "restore path contains a non-normal component".into(),
            ));
        };
        current.push(name);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if is_reparse_point(&metadata) {
                return Err(BackupError::RestoreDestinationChanged);
            }
        }
    }
    Ok(())
}

fn regular_hash(path: &Path) -> Result<String, BackupError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || is_reparse_point(&metadata) {
        return Err(BackupError::Invalid(format!(
            "{} is not an ordinary file",
            path.display()
        )));
    }
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

fn copy_file_verified(
    source: &Path,
    destination: &Path,
    size: u64,
    expected_hash: &str,
) -> Result<(), BackupError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let result = (|| {
        let copied = io::copy(&mut File::open(source)?, &mut output)?;
        output.sync_all()?;
        if copied != size {
            return Err(BackupError::Integrity(format!(
                "copied size mismatch for {}",
                source.display()
            )));
        }
        drop(output);
        let actual = regular_hash(destination)?;
        if actual != expected_hash {
            return Err(BackupError::Integrity(format!(
                "copied hash mismatch for {}",
                source.display()
            )));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

fn tree_digest(entries: &[ManifestEntry]) -> String {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"custodian-backup-tree-v1");
    for entry in sorted {
        let path = entry.relative_path.to_string_lossy().replace('\\', "/");
        let kind = match entry.kind {
            EntryKind::File => b"file".as_slice(),
            EntryKind::Directory => b"directory".as_slice(),
        };
        hash_field(&mut hasher, path.as_bytes());
        hash_field(&mut hasher, kind);
        hash_field(&mut hasher, &entry.size.to_le_bytes());
        hash_field(
            &mut hasher,
            entry.hash.as_deref().unwrap_or_default().as_bytes(),
        );
    }
    hasher.finalize().to_hex().to_string()
}

fn hash_field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn hex_digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn read_manifest(version_dir: &Path) -> Result<(Manifest, String), BackupError> {
    let bytes = fs::read(version_dir.join(MANIFEST_FILE))?;
    let actual = hex_digest(&bytes);
    let expected = fs::read_to_string(version_dir.join(MANIFEST_HASH_FILE))?
        .trim()
        .to_string();
    if expected != actual {
        return Err(BackupError::Integrity(
            "manifest digest does not match manifest bytes".into(),
        ));
    }
    Ok((serde_json::from_slice(&bytes)?, actual))
}

fn verify_version_files(
    version_dir: &Path,
    owner_id: &str,
    expected_state: VersionState,
) -> Result<(), BackupError> {
    let state: StateRecord = read_json(&version_dir.join(STATE_FILE))?;
    if state.owner_id != owner_id || state.state != expected_state {
        return Err(BackupError::Integrity("invalid version state".into()));
    }
    let (manifest, _) = read_manifest(version_dir)?;
    if manifest.schema != 1
        || manifest.owner_id != owner_id
        || manifest.version_id != state.version_id
        || manifest.created_unix != state.created_unix
    {
        return Err(BackupError::Integrity("manifest identity mismatch".into()));
    }
    let allowed_top_level =
        BTreeSet::from([STATE_FILE, MANIFEST_FILE, MANIFEST_HASH_FILE, FILES_DIR]);
    for child in fs::read_dir(version_dir)? {
        let name = child?.file_name();
        if !allowed_top_level.contains(name.to_string_lossy().as_ref()) {
            return Err(BackupError::Integrity(
                "version contains an unexpected top-level entry".into(),
            ));
        }
    }
    if manifest.entries.is_empty() || manifest.entries.len() > MAX_ENTRIES {
        return Err(BackupError::Integrity(
            "invalid manifest entry count".into(),
        ));
    }
    let files_dir = version_dir.join(FILES_DIR);
    let mut expected = BTreeMap::new();
    for entry in &manifest.entries {
        ensure_relative(&entry.relative_path)?;
        if expected
            .insert(entry.relative_path.clone(), entry.kind)
            .is_some()
        {
            return Err(BackupError::Integrity(
                "manifest contains duplicate paths".into(),
            ));
        }
        let payload = safe_join(&files_dir, &entry.relative_path)?;
        let metadata = fs::symlink_metadata(&payload).map_err(|_| {
            BackupError::Integrity(format!(
                "manifest payload is missing: {}",
                entry.relative_path.display()
            ))
        })?;
        match entry.kind {
            EntryKind::Directory => {
                if !metadata.is_dir() || is_reparse_point(&metadata) {
                    return Err(BackupError::Integrity(format!(
                        "manifest directory is not a directory: {}",
                        entry.relative_path.display()
                    )));
                }
            }
            EntryKind::File => {
                if !metadata.is_file() || is_reparse_point(&metadata) {
                    return Err(BackupError::Integrity(format!(
                        "manifest file is not an ordinary file: {}",
                        entry.relative_path.display()
                    )));
                }
                let expected_hash = entry
                    .hash
                    .as_deref()
                    .ok_or_else(|| BackupError::Integrity("file has no hash".into()))?;
                if metadata.len() != entry.size || regular_hash(&payload)? != expected_hash {
                    return Err(BackupError::Integrity(format!(
                        "manifest bytes do not match: {}",
                        entry.relative_path.display()
                    )));
                }
            }
        }
    }
    let actual = collect_payload_paths(&files_dir)?;
    if actual != expected {
        return Err(BackupError::Integrity(
            "payload tree does not exactly match the manifest".into(),
        ));
    }
    if manifest.tree_digest != tree_digest(&manifest.entries) {
        return Err(BackupError::Integrity("tree digest mismatch".into()));
    }
    Ok(())
}

fn collect_payload_paths(root: &Path) -> Result<BTreeMap<PathBuf, EntryKind>, BackupError> {
    let mut result = BTreeMap::new();
    if !root.is_dir() {
        return Err(BackupError::Integrity(
            "payload directory is missing".into(),
        ));
    }
    collect_payload_paths_inner(root, root, &mut result)?;
    Ok(result)
}

fn collect_payload_paths_inner(
    root: &Path,
    current: &Path,
    output: &mut BTreeMap<PathBuf, EntryKind>,
) -> Result<(), BackupError> {
    let mut children = fs::read_dir(current)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    children.sort();
    for child in children {
        let metadata = fs::symlink_metadata(&child)?;
        if is_reparse_point(&metadata) {
            return Err(BackupError::Integrity("payload contains a symlink".into()));
        }
        let relative = child
            .strip_prefix(root)
            .map_err(|_| BackupError::Integrity("payload path escaped root".into()))?;
        ensure_relative(relative)?;
        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else if metadata.is_file() {
            EntryKind::File
        } else {
            return Err(BackupError::Integrity(
                "payload contains an unsupported object".into(),
            ));
        };
        output.insert(relative.to_path_buf(), kind);
        if kind == EntryKind::Directory {
            collect_payload_paths_inner(root, &child, output)?;
        }
    }
    Ok(())
}

fn compare_tree(destination: &Path, manifest: &Manifest) -> TreeVerification {
    let mut expected = BTreeMap::new();
    for entry in &manifest.entries {
        expected.insert(entry.relative_path.clone(), entry);
    }
    let actual = collect_tree_entries(destination).unwrap_or_default();
    let mut result = TreeVerification::default();
    for (path, entry) in &expected {
        let Some(kind) = actual.get(path) else {
            result.missing.push(path.clone());
            continue;
        };
        let target = destination.join(path);
        let matches = match (entry.kind, *kind) {
            (EntryKind::Directory, EntryKind::Directory) => true,
            (EntryKind::File, EntryKind::File) => {
                entry
                    .hash
                    .as_deref()
                    .is_some_and(|hash| regular_hash(&target).ok().as_deref() == Some(hash))
                    && fs::metadata(&target)
                        .map(|metadata| metadata.len() == entry.size)
                        .unwrap_or(false)
            }
            _ => false,
        };
        if !matches {
            result.changed.push(path.clone());
        }
    }
    for path in actual.keys() {
        if !expected.contains_key(path) {
            result.extra.push(path.clone());
        }
    }
    result.missing.sort();
    result.changed.sort();
    result.extra.sort();
    result.exact =
        result.missing.is_empty() && result.changed.is_empty() && result.extra.is_empty();
    result
}

fn collect_tree_entries(path: &Path) -> Result<BTreeMap<PathBuf, EntryKind>, BackupError> {
    let mut result = BTreeMap::new();
    if !path.is_dir() {
        return Ok(result);
    }
    collect_payload_paths_inner(path, path, &mut result)?;
    Ok(result)
}

fn format_tree_error(tree: &TreeVerification) -> String {
    format!(
        "missing={}, changed={}, extra={}",
        tree.missing.len(),
        tree.changed.len(),
        tree.extra.len()
    )
}

fn validate_restore_destination(plan: &RestorePlan) -> Result<(), BackupError> {
    let parent = plan
        .destination
        .parent()
        .ok_or_else(|| BackupError::Invalid("restore destination needs a parent".into()))?;
    if file_identity(&canonical_dir(parent)?)? != plan.destination_parent_identity {
        return Err(BackupError::RestoreDestinationChanged);
    }
    match plan.mode {
        RestoreMode::NewLocation => {
            if plan.destination.exists() {
                return Err(BackupError::RestoreDestinationChanged);
            }
        }
        RestoreMode::PreserveExisting => {
            if let Some(identity) = plan.destination_identity {
                if directory_identity(&plan.destination)? != identity {
                    return Err(BackupError::RestoreDestinationChanged);
                }
            } else if plan.destination.exists() {
                return Err(BackupError::RestoreDestinationChanged);
            }
        }
    }
    Ok(())
}

fn path_name(path: &Path) -> Result<String, BackupError> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| BackupError::Invalid("version directory has no name".into()))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, BackupError> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn write_json_new<T: Serialize>(path: &Path, value: &T) -> Result<(), BackupError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), BackupError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_bytes_atomic(path, &bytes)
}

fn write_text_atomic(path: &Path, value: &str) -> Result<(), BackupError> {
    write_bytes_atomic(path, value.as_bytes())
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        Ok::<(), io::Error>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(BackupError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn approve_plan(plan: &BackupPlan) -> (Approval, Verifier, u64) {
        let key = custodian_approval::ApprovalKey::generate();
        let now = now_unix();
        (
            key.sign(&plan.digest(), now, 300),
            Verifier::new(key.verifying_key()),
            now,
        )
    }

    fn approve_restore(plan: &RestorePlan) -> (Approval, Verifier, u64) {
        let key = custodian_approval::ApprovalKey::generate();
        let now = now_unix();
        (
            key.sign(&plan.digest(), now, 300),
            Verifier::new(key.verifying_key()),
            now,
        )
    }

    fn approve_retention(plan: &RetentionPlan) -> (Approval, Verifier, u64) {
        let key = custodian_approval::ApprovalKey::generate();
        let now = now_unix();
        (
            key.sign(&plan.digest(), now, 300),
            Verifier::new(key.verifying_key()),
            now,
        )
    }

    fn make_source(root: &Path, suffix: &[u8]) -> PathBuf {
        let source = root.join("selected");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("a.txt"), b"alpha").unwrap();
        fs::write(source.join("nested").join("b.bin"), suffix).unwrap();
        source
    }

    #[test]
    fn backup_and_restore_verify_the_complete_tree_and_bytes() {
        let fixture = tempfile::tempdir().unwrap();
        let source = make_source(fixture.path(), b"beta");
        let store = BackupStore::new(fixture.path().join("backups")).unwrap();
        let plan = store
            .prepare_backup(&[source], RetentionPolicy::default())
            .unwrap();
        assert_eq!(plan.total_bytes(), 9);
        assert!(!store.owner_id().is_empty());
        let (approval, verifier, now) = approve_plan(&plan);
        let version = store
            .create_version(&plan, &approval, &verifier, now, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(version.state, VersionState::Verified);
        let verified = store.verify_version(&version.id).unwrap();
        assert_eq!(verified.entries(), 4);
        assert!(!verified.manifest_digest().is_empty());

        let destination = fixture.path().join("restored");
        let restore = store
            .prepare_restore_to_new_location(&version.id, &destination)
            .unwrap();
        let (approval, verifier, now) = approve_restore(&restore);
        let outcome = store
            .restore(&restore, &approval, &verifier, now, &AtomicBool::new(false))
            .unwrap();
        assert!(outcome.verified);
        assert_eq!(
            fs::read(destination.join("selected/a.txt")).unwrap(),
            b"alpha"
        );
        assert_eq!(
            fs::read(destination.join("selected/nested/b.bin")).unwrap(),
            b"beta"
        );
        assert!(
            store
                .verify_restored(&version.id, &destination)
                .unwrap()
                .exact
        );
    }

    #[test]
    fn interrupted_and_corrupted_versions_are_not_healthy() {
        let fixture = tempfile::tempdir().unwrap();
        let source = make_source(fixture.path(), b"beta");
        let store = BackupStore::new(fixture.path().join("backups")).unwrap();
        let plan = store
            .prepare_backup(&[source], RetentionPolicy::default())
            .unwrap();
        let (approval, verifier, now) = approve_plan(&plan);
        assert!(matches!(
            store.create_version(&plan, &approval, &verifier, now, &AtomicBool::new(true)),
            Err(BackupError::Interrupted)
        ));
        assert!(matches!(
            store.verify_version(&plan.id),
            Err(BackupError::Incomplete)
        ));

        let plan = store
            .prepare_backup(
                &[fixture.path().join("selected")],
                RetentionPolicy::default(),
            )
            .unwrap();
        let (approval, verifier, now) = approve_plan(&plan);
        let version = store
            .create_version(&plan, &approval, &verifier, now, &AtomicBool::new(false))
            .unwrap();
        let payload = store
            .root()
            .join(VERSIONS_DIR)
            .join(version.id)
            .join(FILES_DIR)
            .join("selected/a.txt");
        fs::write(payload, b"corrupted").unwrap();
        assert!(matches!(
            store.verify_version(&plan.id),
            Err(BackupError::Integrity(_))
        ));
    }

    #[test]
    fn changed_backup_destination_is_refused_before_writing_a_version() {
        let fixture = tempfile::tempdir().unwrap();
        let source = make_source(fixture.path(), b"beta");
        let root = fixture.path().join("backups");
        let store = BackupStore::new(&root).unwrap();
        let plan = store
            .prepare_backup(&[source], RetentionPolicy::default())
            .unwrap();
        fs::rename(&root, fixture.path().join("moved-backups")).unwrap();
        fs::create_dir(&root).unwrap();
        let (approval, verifier, now) = approve_plan(&plan);
        assert!(matches!(
            store.create_version(&plan, &approval, &verifier, now, &AtomicBool::new(false)),
            Err(BackupError::DestinationChanged)
        ));
    }

    #[test]
    fn changed_restore_destination_is_refused_without_overwriting_it() {
        let fixture = tempfile::tempdir().unwrap();
        let source = make_source(fixture.path(), b"beta");
        let store = BackupStore::new(fixture.path().join("backups")).unwrap();
        let plan = store
            .prepare_backup(&[source], RetentionPolicy::default())
            .unwrap();
        let (approval, verifier, now) = approve_plan(&plan);
        let version = store
            .create_version(&plan, &approval, &verifier, now, &AtomicBool::new(false))
            .unwrap();
        let destination = fixture.path().join("restore");
        let restore = store
            .prepare_restore(&version.id, &destination, RestoreMode::NewLocation)
            .unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("sentinel.txt"), b"newer destination").unwrap();
        let (approval, verifier, now) = approve_restore(&restore);
        assert!(matches!(
            store.restore(&restore, &approval, &verifier, now, &AtomicBool::new(false)),
            Err(BackupError::RestoreDestinationChanged)
        ));
        assert_eq!(
            fs::read(destination.join("sentinel.txt")).unwrap(),
            b"newer destination"
        );
    }

    #[test]
    fn preserve_existing_restore_keeps_a_newer_destination_edit() {
        let fixture = tempfile::tempdir().unwrap();
        let source = make_source(fixture.path(), b"beta");
        let store = BackupStore::new(fixture.path().join("backups")).unwrap();
        let plan = store
            .prepare_backup(&[source], RetentionPolicy::default())
            .unwrap();
        let (approval, verifier, now) = approve_plan(&plan);
        let version = store
            .create_version(&plan, &approval, &verifier, now, &AtomicBool::new(false))
            .unwrap();
        let destination = fixture.path().join("restore");
        fs::create_dir_all(destination.join("selected")).unwrap();
        fs::write(destination.join("selected/a.txt"), b"newer edit").unwrap();
        let restore = store
            .prepare_restore(&version.id, &destination, RestoreMode::PreserveExisting)
            .unwrap();
        let (approval, verifier, now) = approve_restore(&restore);
        let outcome = store
            .restore(&restore, &approval, &verifier, now, &AtomicBool::new(false))
            .unwrap();
        assert!(!outcome.verified);
        assert!(outcome
            .preserved_newer
            .contains(&PathBuf::from("selected/a.txt")));
        assert_eq!(
            fs::read(destination.join("selected/a.txt")).unwrap(),
            b"newer edit"
        );
        assert_eq!(
            fs::read(destination.join("selected/nested/b.bin")).unwrap(),
            b"beta"
        );
    }

    #[test]
    fn retention_removes_old_healthy_versions_but_preserves_only_healthy_recovery() {
        let fixture = tempfile::tempdir().unwrap();
        let source = make_source(fixture.path(), b"one");
        let store = BackupStore::new(fixture.path().join("backups")).unwrap();
        let mut healthy = Vec::new();
        for body in [b"one".as_slice(), b"two".as_slice()] {
            fs::write(source.join("nested/b.bin"), body).unwrap();
            let plan = store
                .prepare_backup(&[source.clone()], RetentionPolicy::default())
                .unwrap();
            let (approval, verifier, now) = approve_plan(&plan);
            healthy.push(
                store
                    .create_version(&plan, &approval, &verifier, now, &AtomicBool::new(false))
                    .unwrap(),
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        let corrupt_id = {
            let plan = store
                .prepare_backup(&[source], RetentionPolicy::default())
                .unwrap();
            let (approval, verifier, now) = approve_plan(&plan);
            let version = store
                .create_version(&plan, &approval, &verifier, now, &AtomicBool::new(false))
                .unwrap();
            let payload = store
                .root()
                .join(VERSIONS_DIR)
                .join(&version.id)
                .join(FILES_DIR)
                .join("selected/a.txt");
            fs::write(payload, b"corrupted").unwrap();
            version.id
        };
        let retention = store.prepare_retention(1).unwrap();
        assert_eq!(retention.candidates.len(), 1);
        let (approval, verifier, now) = approve_retention(&retention);
        let outcome = store
            .retain_verified(&retention, &approval, &verifier, now)
            .unwrap();
        assert_eq!(outcome.removed.len(), 1);
        assert_eq!(
            store
                .list_versions()
                .unwrap()
                .into_iter()
                .filter(|summary| store.verify_version(&summary.id).is_ok())
                .count(),
            1
        );
        assert!(store.root().join(VERSIONS_DIR).join(corrupt_id).exists());
        assert!(healthy.iter().any(|version| {
            store.root().join(VERSIONS_DIR).join(&version.id).exists()
                && store.verify_version(&version.id).is_ok()
        }));
    }

    #[test]
    fn wrong_backup_authority_cannot_create_a_version() {
        let fixture = tempfile::tempdir().unwrap();
        let source = make_source(fixture.path(), b"beta");
        let store = BackupStore::new(fixture.path().join("backups")).unwrap();
        let plan = store
            .prepare_backup(&[source], RetentionPolicy::default())
            .unwrap();
        let wrong = custodian_approval::ApprovalKey::generate();
        let now = now_unix();
        let approval = wrong.sign(&[9u8; 32], now, 300);
        let verifier = Verifier::new(wrong.verifying_key());
        assert!(matches!(
            store.create_version(&plan, &approval, &verifier, now, &AtomicBool::new(false)),
            Err(BackupError::NotAuthorized(ApprovalError::Auth))
        ));
    }
}
