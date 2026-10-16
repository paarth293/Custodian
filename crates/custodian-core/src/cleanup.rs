//! Safe, undoable clean-up: files are moved into Custodian's quarantine
//! ("the safe place") instead of being deleted, with a manifest that lets
//! every action be reversed and checked by hash (Custodian Contract, D-06).
//!
//! Before moving a duplicate, the copy being kept is re-hashed. A plan can
//! therefore never remove the last copy of a file, even if files changed
//! between the scan and the approval.
//!
//! A file in the safe place still takes up space (EV-01). Its space frees
//! when its quarantine window ends and it is purged, and a purge only happens
//! after checking again that the copy in the safe place is unchanged and that
//! the identical copy that was kept is still there (K-29). If anything does
//! not check out, the file stays. Undo works before and after a purge: a
//! purged copy is re-created from the kept original, verified by hash. The
//! record of every clean-up stays forever.

use crate::duplicates::DuplicateGroup;
use crate::hashing::hash_file;
use custodian_approval::{Approval, ApprovalError, PlanHash, Verifier};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const MANIFEST: &str = "manifest.json";

/// How long files stay in the safe place before their space is freed (§27, rung 3).
pub const QUARANTINE_DAYS: u64 = 30;

const DAY_SECS: u64 = 24 * 60 * 60;

/// Most "(restored N)" names tried when the original spot is taken.
const MAX_RESTORE_NAMES: usize = 50;

#[derive(Debug, Error)]
pub enum CleanupError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("manifest error: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("unknown clean-up {0}")]
    UnknownPlan(String),
    #[error("not approved: {0}")]
    NotApproved(#[from] ApprovalError),
}

/// One file to move into quarantine because an identical `keep` copy exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanItem {
    pub path: PathBuf,
    pub keep: PathBuf,
    pub size: u64,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPlan {
    pub id: String,
    pub items: Vec<PlanItem>,
}

/// Copies a clean-up left alone because they are in a read-only place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeftOut {
    pub files: usize,
    pub bytes: u64,
    /// The places, such as "OneDrive", for telling the person why.
    pub places: Vec<String>,
}

/// The clean-up items for duplicate groups. One copy of each group always
/// stays, and copies in a read-only place (a cloud-synced folder, where a
/// removal would reach the cloud and other devices) are never proposed
/// (K-14). `read_only` names the place a path is in, if it is read-only.
///
/// When one copy is in such a place, that is the copy kept: it cannot be
/// touched anyway, so the extra copies elsewhere can still go.
pub fn duplicate_plan_items(
    groups: &[DuplicateGroup],
    read_only: &dyn Fn(&Path) -> Option<String>,
) -> (Vec<PlanItem>, LeftOut) {
    let mut items = Vec::new();
    let mut left = LeftOut::default();
    for group in groups.iter().filter(|g| g.files.len() >= 2) {
        let places: Vec<Option<String>> = group.files.iter().map(|f| read_only(&f.path)).collect();
        let keep_index = places.iter().position(Option::is_some).unwrap_or(0);
        let keep = &group.files[keep_index].path;
        for (index, file) in group.files.iter().enumerate() {
            if index == keep_index {
                continue;
            }
            if let Some(place) = &places[index] {
                left.files += 1;
                left.bytes += group.size;
                if !left.places.contains(place) {
                    left.places.push(place.clone());
                }
                continue;
            }
            items.push(PlanItem {
                path: file.path.clone(),
                keep: keep.clone(),
                size: group.size,
                hash: group.hash.clone(),
            });
        }
    }
    (items, left)
}

impl CleanupPlan {
    pub fn new(items: Vec<PlanItem>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
            items,
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.items.iter().map(|i| i.size).sum()
    }

    /// Canonical digest of everything the plan will do. Approvals sign this,
    /// so changing any path, size or hash invalidates them.
    pub fn digest(&self) -> PlanHash {
        fn field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
            hasher.update(&(bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
        let mut hasher = blake3::Hasher::new();
        field(&mut hasher, b"custodian-plan-v1");
        field(&mut hasher, self.id.as_bytes());
        for item in &self.items {
            field(&mut hasher, item.path.as_os_str().as_encoded_bytes());
            field(&mut hasher, item.keep.as_os_str().as_encoded_bytes());
            field(&mut hasher, &item.size.to_le_bytes());
            field(&mut hasher, item.hash.as_bytes());
        }
        hasher.finalize().into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ManifestEntry {
    original: PathBuf,
    stored: PathBuf,
    size: u64,
    hash: String,
    /// The identical copy that stayed where it was. Clean-ups recorded before
    /// this was kept have none, so they are never purged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    keep: Option<PathBuf>,
    /// The kept copy's modification time at clean-up, so a copy that cloud
    /// storage has since moved online can be checked without downloading it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    keep_modified: Option<u64>,
    /// Where undo put the file back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    restored_to: Option<PathBuf>,
    /// When the copy in the safe place was deleted to free its space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    purged_unix: Option<u64>,
}

impl ManifestEntry {
    /// Still in the safe place: neither put back nor purged.
    fn in_safe_place(&self) -> bool {
        self.restored_to.is_none() && self.purged_unix.is_none() && self.stored.exists()
    }

    /// Purged and not put back since.
    fn purged(&self) -> bool {
        self.purged_unix.is_some() && self.restored_to.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Manifest {
    plan_id: String,
    created_unix: u64,
    #[serde(default)]
    completed_order: Option<u64>,
    /// Set once an undo has put back everything it could.
    undone: bool,
    entries: Vec<ManifestEntry>,
}

impl Manifest {
    /// When the files' quarantine window ends.
    fn frees_unix(&self) -> u64 {
        self.created_unix.saturating_add(QUARANTINE_DAYS * DAY_SECS)
    }
}

/// Result of executing a plan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CleanupOutcome {
    pub plan_id: String,
    pub moved: usize,
    pub bytes: u64,
    /// When the moved files' space frees up, unless they are put back first.
    pub frees_unix: u64,
    /// Items left untouched, with the reason.
    pub skipped: Vec<(PathBuf, String)>,
}

/// Result of undoing a plan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UndoOutcome {
    pub plan_id: String,
    pub restored: usize,
    /// Files put back beside their original spot because something else is
    /// there now: (original, where it went).
    pub beside: Vec<(PathBuf, PathBuf)>,
    pub skipped: Vec<(PathBuf, String)>,
}

/// Result of freeing space in the safe place.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PurgeOutcome {
    pub purged: usize,
    pub bytes: u64,
    /// Files kept because purging them could lose something, with the reason.
    pub kept: Vec<(PathBuf, String)>,
}

/// One past clean-up, for the history view.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HistoryEntry {
    pub plan_id: String,
    pub created_unix: u64,
    pub files: usize,
    pub bytes: u64,
    /// Every file that could come back has been put back.
    pub undone: bool,
    /// Files still in the safe place, and their size.
    pub waiting_files: usize,
    pub waiting_bytes: u64,
    /// When the waiting files' space frees up.
    pub frees_unix: u64,
    /// Files whose space has been freed.
    pub purged_files: usize,
    pub purged_bytes: u64,
}

/// The quarantine folder and its manifests.
pub struct Quarantine {
    root: PathBuf,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn modified_unix(meta: &fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// A cloud file whose bytes are not (all) on this PC. Reading it would make
/// the sync client download it, which Custodian never does (K-20).
#[cfg(windows)]
fn is_cloud_placeholder(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
    const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
    meta.file_attributes()
        & (FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
        != 0
}

#[cfg(not(windows))]
fn is_cloud_placeholder(_meta: &fs::Metadata) -> bool {
    false
}

/// Moves a file without ever replacing one, falling back to copy + verify +
/// delete across volumes.
fn move_verified(from: &Path, to: &Path, hash: &str) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    let current_hash = hash_file(from)?;
    if current_hash != hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("stored payload integrity check failed: expected {hash}, found {current_hash}"),
        ));
    }
    custodian_winsec::fsops::move_no_replace(from, to, |copy| Ok(hash_file(copy)? == hash))
}

/// Copies `from` into a new file at `to` (never replacing one), checks the
/// copy against `hash`, and gives it `from`'s modification time.
fn copy_verified(from: &Path, to: &Path, hash: &str) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = OpenOptions::new().write(true).create_new(true).open(to)?;
    let copied = io::copy(&mut File::open(from)?, &mut out).and_then(|_| out.sync_all());
    let result = copied.and_then(|()| {
        if let Ok(modified) = fs::metadata(from).and_then(|m| m.modified()) {
            let _ = out.set_modified(modified);
        }
        drop(out);
        if hash_file(to)? == hash {
            Ok(())
        } else {
            Err(io::Error::other("copy verification failed"))
        }
    });
    if result.is_err() {
        let _ = fs::remove_file(to);
    }
    result
}

/// `original`, then `name (restored).ext`, `name (restored 2).ext`, …: the
/// places undo may put a file back, in order (K-17: never overwrite).
fn restore_targets(original: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    let stem = original
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = original
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    std::iter::once(original.to_path_buf()).chain((1..MAX_RESTORE_NAMES).map(move |n| {
        let label = if n == 1 {
            "restored".to_string()
        } else {
            format!("restored {n}")
        };
        original.with_file_name(format!("{stem} ({label}){ext}"))
    }))
}

/// Puts a file back with `place` (a move or a copy that never replaces
/// anything), at the original path or beside it. Returns where it went.
fn put_back(
    original: &Path,
    mut place: impl FnMut(&Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
    for target in restore_targets(original) {
        match place(&target) {
            Ok(()) => return Ok(target),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "every name next to the original is taken",
    ))
}

/// Checks that the copy that was kept is still there and identical. A cloud
/// placeholder is checked by size and time, so it is never downloaded.
fn kept_copy_intact(entry: &ManifestEntry) -> Result<&Path, String> {
    let keep = entry
        .keep
        .as_deref()
        .ok_or("there is no record of the copy that was kept")?;
    let changed = || "the copy that was kept has changed".to_string();
    // Not followed if it became a link: only a real file counts as a copy.
    let meta =
        fs::symlink_metadata(keep).map_err(|_| "the copy that was kept is gone".to_string())?;
    if !meta.file_type().is_file() || meta.len() != entry.size {
        return Err(changed());
    }
    if is_cloud_placeholder(&meta) {
        return Err(
            "the copy that was kept is a cloud placeholder and cannot be verified locally offline"
                .to_string(),
        );
    }
    match hash_file(keep) {
        Ok(h) if h == entry.hash => Ok(keep),
        _ => Err(changed()),
    }
}

/// Whether the copy in the safe place may be deleted: it must be exactly
/// what was put there, and an identical copy must still exist (K-29).
fn purgeable(entry: &ManifestEntry) -> Result<(), String> {
    match hash_file(&entry.stored) {
        Ok(h) if h == entry.hash => {}
        Ok(_) => return Err("the copy in the safe place has changed".into()),
        Err(err) => return Err(format!("I couldn't read the copy in the safe place: {err}")),
    }
    kept_copy_intact(entry).map(|_| ())
}

impl Quarantine {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn plan_dir(&self, plan_id: &str) -> PathBuf {
        self.root.join(plan_id)
    }

    fn write_manifest(&self, manifest: &Manifest) -> Result<(), CleanupError> {
        let dir = self.plan_dir(&manifest.plan_id);
        fs::create_dir_all(&dir)?;
        let tmp = dir.join("manifest.json.tmp");
        let bytes = serde_json::to_vec_pretty(manifest)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        std::io::Write::write_all(&mut file, &bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(tmp, dir.join(MANIFEST))?;
        Ok(())
    }

    fn is_valid_plan_id(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 64
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    fn read_manifest(&self, plan_id: &str) -> Result<Manifest, CleanupError> {
        if !Self::is_valid_plan_id(plan_id) {
            return Err(CleanupError::UnknownPlan(format!(
                "invalid plan id format: {plan_id}"
            )));
        }
        let expected_dir = self.plan_dir(plan_id);
        let path = expected_dir.join(MANIFEST);
        let bytes = fs::read(&path).map_err(|_| CleanupError::UnknownPlan(plan_id.to_string()))?;
        let manifest: Manifest = serde_json::from_slice(&bytes)?;
        if manifest.plan_id != plan_id {
            return Err(CleanupError::UnknownPlan(format!(
                "manifest plan_id mismatch: {} vs {}",
                manifest.plan_id, plan_id
            )));
        }
        for entry in &manifest.entries {
            let confined = entry
                .stored
                .strip_prefix(&expected_dir)
                .is_ok_and(|relative| {
                    relative.components().next().is_some()
                        && relative
                            .components()
                            .all(|part| matches!(part, std::path::Component::Normal(_)))
                });
            if !confined {
                return Err(CleanupError::UnknownPlan(format!(
                    "manifest contains stored path outside vault root: {}",
                    entry.stored.display()
                )));
            }
        }
        Ok(manifest)
    }

    /// Every clean-up, newest first.
    fn manifests(&self) -> Vec<Manifest> {
        let Ok(dirs) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut all: Vec<Manifest> = dirs
            .flatten()
            .filter_map(|d| {
                let name = d.file_name();
                let id = name.to_str()?;
                self.read_manifest(id).ok()
            })
            .collect();
        all.sort_by(|a, b| {
            b.created_unix
                .cmp(&a.created_unix)
                .then(b.plan_id.cmp(&a.plan_id))
        });
        all
    }

    /// Runs `plan` only if `approval` signs exactly this plan, has not expired
    /// and was never used before (SR-D1).
    pub fn execute_approved(
        &self,
        plan: &CleanupPlan,
        approval: &Approval,
        verifier: &Verifier,
        now_unix: u64,
    ) -> Result<CleanupOutcome, CleanupError> {
        verifier.verify(&plan.digest(), approval, now_unix)?;
        self.execute(plan)
    }

    /// Moves every item of `plan` into quarantine. Items whose file or kept
    /// copy changed since the scan are skipped, never forced. Private: plans
    /// run only through [`Quarantine::execute_approved`].
    fn execute(&self, plan: &CleanupPlan) -> Result<CleanupOutcome, CleanupError> {
        let _mutation = crate::mutation_lock::MUTATIONS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut manifest = Manifest {
            plan_id: plan.id.clone(),
            created_unix: now_unix(),
            completed_order: None,
            undone: false,
            entries: Vec::new(),
        };
        let mut outcome = CleanupOutcome {
            plan_id: plan.id.clone(),
            frees_unix: manifest.frees_unix(),
            ..Default::default()
        };
        let files_dir = self.plan_dir(&plan.id).join("files");

        for (index, item) in plan.items.iter().enumerate() {
            let skip = |reason: &str| (item.path.clone(), reason.to_string());
            if item.path == item.keep {
                outcome
                    .skipped
                    .push(skip("the file to remove is the kept copy"));
                continue;
            }
            match hash_file(&item.keep) {
                Ok(h) if h == item.hash => {}
                _ => {
                    outcome
                        .skipped
                        .push(skip("the copy to keep changed or is missing"));
                    continue;
                }
            }
            match hash_file(&item.path) {
                Ok(h) if h == item.hash => {}
                _ => {
                    outcome.skipped.push(skip("the file changed or is missing"));
                    continue;
                }
            }
            let keep_modified = fs::metadata(&item.keep)
                .ok()
                .as_ref()
                .and_then(modified_unix);
            let name = item
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let stored = files_dir.join(format!("{index:05}_{name}"));
            manifest.entries.push(ManifestEntry {
                original: item.path.clone(),
                stored: stored.clone(),
                size: item.size,
                hash: item.hash.clone(),
                keep: Some(item.keep.clone()),
                keep_modified,
                restored_to: None,
                purged_unix: None,
            });
            // Persist intent before moving so a crash never loses track of a file.
            self.write_manifest(&manifest)?;
            if let Err(err) = move_verified(&item.path, &stored, &item.hash) {
                // A failed rollback can leave the payload at `stored`. Keep its
                // prepared entry so recovery never loses the file's location.
                if !stored.exists() {
                    manifest.entries.pop();
                }
                let _ = self.write_manifest(&manifest);
                outcome.skipped.push(skip(&err.to_string()));
                continue;
            }
            outcome.moved += 1;
            outcome.bytes += item.size;
        }
        if !manifest.entries.is_empty() {
            manifest.completed_order = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
                    .min(u64::MAX as u128) as u64,
            );
            self.write_manifest(&manifest)?;
        }
        Ok(outcome)
    }

    /// Puts every file of a past clean-up back where it was, verified by
    /// hash. A file whose spot is taken is put back beside it, never over it.
    /// A file whose space was already freed is re-created from the kept
    /// original, if that is still identical.
    pub fn undo(&self, plan_id: &str) -> Result<UndoOutcome, CleanupError> {
        let _mutation = crate::mutation_lock::MUTATIONS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut manifest = self.read_manifest(plan_id)?;
        let mut outcome = UndoOutcome {
            plan_id: plan_id.to_string(),
            ..Default::default()
        };
        for i in 0..manifest.entries.len() {
            let entry = &manifest.entries[i];
            let result = if entry.in_safe_place() {
                put_back(&entry.original, |target| {
                    move_verified(&entry.stored, target, &entry.hash)
                })
                .map_err(|e| e.to_string())
            } else if entry.purged() {
                kept_copy_intact(entry)
                    .map_err(|reason| format!("its space was already freed and {reason}"))
                    .and_then(|keep| {
                        put_back(&entry.original, |target| {
                            copy_verified(keep, target, &entry.hash)
                        })
                        .map_err(|e| e.to_string())
                    })
            } else {
                continue; // Already put back.
            };
            match result {
                Ok(to) => {
                    if to != entry.original {
                        outcome.beside.push((entry.original.clone(), to.clone()));
                    }
                    manifest.entries[i].restored_to = Some(to);
                    outcome.restored += 1;
                    // Persist after every file so a crash never loses track.
                    self.write_manifest(&manifest)?;
                }
                Err(reason) => outcome.skipped.push((entry.original.clone(), reason)),
            }
        }
        manifest.undone = outcome.skipped.is_empty();
        self.write_manifest(&manifest)?;
        Ok(outcome)
    }

    /// Undoes the most recent clean-up that still has files to put back.
    pub fn undo_last(&self) -> Result<UndoOutcome, CleanupError> {
        let last = self
            .manifests()
            .into_iter()
            .find(|m| !m.undone && m.entries.iter().any(|e| e.in_safe_place() || e.purged()))
            .ok_or(CleanupError::NothingToUndo)?;
        self.undo(&last.plan_id)
    }

    /// Frees the space of files whose quarantine window has ended, after
    /// checking each one again (K-29). Anything that does not check out stays.
    pub fn purge_expired(&self, now_unix: u64) -> Result<PurgeOutcome, CleanupError> {
        self.purge(now_unix, |m| now_unix >= m.frees_unix())
    }

    /// Frees the space of every file in the safe place now, with the same
    /// checks. Only for after the person has confirmed: once purged, a file
    /// can only come back as a copy of the original that was kept.
    pub fn purge_now(&self, now_unix: u64) -> Result<PurgeOutcome, CleanupError> {
        self.purge(now_unix, |_| true)
    }

    fn purge(
        &self,
        now_unix: u64,
        due: impl Fn(&Manifest) -> bool,
    ) -> Result<PurgeOutcome, CleanupError> {
        let _mutation = crate::mutation_lock::MUTATIONS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut outcome = PurgeOutcome::default();
        for mut manifest in self.manifests() {
            // A clock that reads earlier than the clean-up itself cannot be
            // trusted to say its window has passed (RT-06).
            if !due(&manifest) || now_unix < manifest.created_unix {
                continue;
            }
            let mut changed = false;
            for entry in manifest.entries.iter_mut() {
                if !entry.in_safe_place() {
                    continue;
                }
                match purgeable(entry)
                    .and_then(|()| fs::remove_file(&entry.stored).map_err(|e| e.to_string()))
                {
                    Ok(()) => {
                        entry.purged_unix = Some(now_unix);
                        outcome.purged += 1;
                        outcome.bytes += entry.size;
                        changed = true;
                    }
                    Err(reason) => outcome.kept.push((entry.original.clone(), reason)),
                }
            }
            if changed {
                self.write_manifest(&manifest)?;
            }
        }
        Ok(outcome)
    }

    /// Files in the safe place now and their total size.
    pub fn waiting(&self) -> (usize, u64) {
        self.manifests()
            .iter()
            .flat_map(|m| m.entries.iter())
            .filter(|e| e.in_safe_place())
            .fold((0, 0), |(n, bytes), e| (n + 1, bytes + e.size))
    }

    pub fn latest_undoable_order(&self) -> Option<u64> {
        self.manifests()
            .into_iter()
            .filter(|manifest| {
                !manifest.undone
                    && manifest
                        .entries
                        .iter()
                        .any(|entry| entry.in_safe_place() || entry.purged())
            })
            .map(|manifest| {
                manifest
                    .completed_order
                    .unwrap_or_else(|| manifest.created_unix.saturating_mul(1_000_000_000))
            })
            .max()
    }

    /// Past clean-ups, newest first.
    pub fn history(&self) -> Vec<HistoryEntry> {
        self.manifests()
            .into_iter()
            .map(|m| {
                let waiting: Vec<&ManifestEntry> =
                    m.entries.iter().filter(|e| e.in_safe_place()).collect();
                let purged: Vec<&ManifestEntry> = m.entries.iter().filter(|e| e.purged()).collect();
                HistoryEntry {
                    files: m.entries.len(),
                    bytes: m.entries.iter().map(|e| e.size).sum(),
                    undone: m.undone,
                    waiting_files: waiting.len(),
                    waiting_bytes: waiting.iter().map(|e| e.size).sum(),
                    frees_unix: m.frees_unix(),
                    purged_files: purged.len(),
                    purged_bytes: purged.iter().map(|e| e.size).sum(),
                    plan_id: m.plan_id,
                    created_unix: m.created_unix,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        quarantine: Quarantine,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("user");
        fs::create_dir_all(&root).unwrap();
        let quarantine = Quarantine::new(dir.path().join("quarantine"));
        Fixture {
            _dir: dir,
            root,
            quarantine,
        }
    }

    fn item(keep: &Path, copy: &Path) -> PlanItem {
        PlanItem {
            path: copy.to_path_buf(),
            keep: keep.to_path_buf(),
            size: fs::metadata(copy).unwrap().len(),
            hash: hash_file(copy).unwrap(),
        }
    }

    /// A kept file and its duplicate, moved to the safe place by a plan.
    fn cleaned(f: &Fixture, name: &str, body: &[u8]) -> (PathBuf, PathBuf, CleanupPlan) {
        let keep = f.root.join(format!("{name}.jpg"));
        let copy = f.root.join(format!("{name} (1).jpg"));
        fs::write(&keep, body).unwrap();
        fs::write(&copy, body).unwrap();
        let plan = CleanupPlan::new(vec![item(&keep, &copy)]);
        assert_eq!(f.quarantine.execute(&plan).unwrap().moved, 1);
        (keep, copy, plan)
    }

    const LATER: u64 = QUARANTINE_DAYS * DAY_SECS + 1;

    #[test]
    fn moves_duplicates_into_quarantine_and_undo_restores_them() {
        let f = fixture();
        let keep = f.root.join("photo.jpg");
        let copy = f.root.join("photo (1).jpg");
        fs::write(&keep, b"picture").unwrap();
        fs::write(&copy, b"picture").unwrap();
        let plan = CleanupPlan::new(vec![item(&keep, &copy)]);

        let outcome = f.quarantine.execute(&plan).unwrap();
        assert_eq!(outcome.moved, 1);
        assert_eq!(outcome.bytes, 7);
        assert!(outcome.frees_unix >= now_unix() + QUARANTINE_DAYS * DAY_SECS - 5);
        assert!(keep.exists());
        assert!(!copy.exists());
        let history = f.quarantine.history();
        assert_eq!(history[0].files, 1);
        assert_eq!(history[0].waiting_files, 1);
        assert_eq!(f.quarantine.waiting(), (1, 7));

        let undo = f.quarantine.undo_last().unwrap();
        assert_eq!(undo.restored, 1);
        assert!(undo.beside.is_empty());
        assert_eq!(fs::read(&copy).unwrap(), b"picture");
        assert!(f.quarantine.history()[0].undone);
        assert_eq!(f.quarantine.waiting(), (0, 0));
        assert!(matches!(
            f.quarantine.undo_last(),
            Err(CleanupError::NothingToUndo)
        ));
    }

    #[test]
    fn never_removes_the_last_copy() {
        let f = fixture();
        let keep = f.root.join("a.txt");
        let copy = f.root.join("b.txt");
        fs::write(&keep, b"same").unwrap();
        fs::write(&copy, b"same").unwrap();
        let plan = CleanupPlan::new(vec![item(&keep, &copy)]);

        // The kept copy changes after the scan: the duplicate must stay.
        fs::write(&keep, b"edited").unwrap();
        let outcome = f.quarantine.execute(&plan).unwrap();
        assert_eq!(outcome.moved, 0);
        assert_eq!(outcome.skipped.len(), 1);
        assert!(copy.exists());
    }

    #[test]
    fn skips_files_that_changed_since_the_scan() {
        let f = fixture();
        let keep = f.root.join("a.txt");
        let copy = f.root.join("b.txt");
        fs::write(&keep, b"same").unwrap();
        fs::write(&copy, b"same").unwrap();
        let plan = CleanupPlan::new(vec![item(&keep, &copy)]);

        fs::write(&copy, b"now different").unwrap();
        let outcome = f.quarantine.execute(&plan).unwrap();
        assert_eq!(outcome.moved, 0);
        assert_eq!(fs::read(&copy).unwrap(), b"now different");
    }

    #[test]
    fn rejects_a_plan_that_targets_the_kept_copy() {
        let f = fixture();
        let keep = f.root.join("a.txt");
        fs::write(&keep, b"x").unwrap();
        let plan = CleanupPlan::new(vec![item(&keep, &keep)]);
        assert_eq!(f.quarantine.execute(&plan).unwrap().moved, 0);
        assert!(keep.exists());
    }

    #[test]
    fn undo_puts_a_file_beside_a_new_one_instead_of_over_it() {
        let f = fixture();
        let (_keep, copy, plan) = cleaned(&f, "b", b"same");

        fs::write(&copy, b"brand new file").unwrap();
        let undo = f.quarantine.undo(&plan.id).unwrap();
        assert_eq!(undo.restored, 1);
        assert_eq!(
            fs::read(&copy).unwrap(),
            b"brand new file",
            "never overwritten"
        );
        let beside = f.root.join("b (1) (restored).jpg");
        assert_eq!(undo.beside, vec![(copy.clone(), beside.clone())]);
        assert_eq!(fs::read(&beside).unwrap(), b"same");
        assert!(f.quarantine.history()[0].undone);

        // Taken names are skipped too.
        let (_k2, copy2, plan2) = cleaned(&f, "c", b"other");
        fs::write(&copy2, b"new").unwrap();
        fs::write(f.root.join("c (1) (restored).jpg"), b"also new").unwrap();
        let undo = f.quarantine.undo(&plan2.id).unwrap();
        assert_eq!(undo.beside[0].1, f.root.join("c (1) (restored 2).jpg"));
    }

    #[test]
    fn a_failed_undo_can_be_tried_again() {
        let f = fixture();
        let (_keep, copy, plan) = cleaned(&f, "song", b"tune");
        // The folder it lived in is replaced by a file, so it cannot go back.
        let dir_as_file = f.root.join("blocked");
        fs::write(&dir_as_file, b"x").unwrap();
        let mut manifest = f.quarantine.read_manifest(&plan.id).unwrap();
        manifest.entries[0].original = dir_as_file.join("song (1).jpg");
        f.quarantine.write_manifest(&manifest).unwrap();

        let undo = f.quarantine.undo_last().unwrap();
        assert_eq!(undo.restored, 0);
        assert_eq!(undo.skipped.len(), 1);
        assert!(!f.quarantine.history()[0].undone);
        assert_eq!(f.quarantine.waiting().0, 1, "still safe, not stranded");

        // Once the way is clear, the same undo finishes the job.
        fs::remove_file(&dir_as_file).unwrap();
        let undo = f.quarantine.undo_last().unwrap();
        assert_eq!(undo.restored, 1);
        assert!(dir_as_file.join("song (1).jpg").exists());
        assert!(!copy.exists());
    }

    #[test]
    fn purges_only_after_the_window_and_only_verified_copies() {
        let f = fixture();
        let (keep, _copy, _plan) = cleaned(&f, "holiday", b"beach");
        let now = now_unix();

        // Before the window ends nothing is purged.
        let early = f.quarantine.purge_expired(now).unwrap();
        assert_eq!(early, PurgeOutcome::default());
        assert_eq!(f.quarantine.waiting().0, 1);

        // After it, the copy goes, because the original is still identical.
        let later = f.quarantine.purge_expired(now + LATER).unwrap();
        assert_eq!(later.purged, 1);
        assert_eq!(later.bytes, 5);
        assert_eq!(f.quarantine.waiting(), (0, 0));
        assert!(keep.exists(), "the original is untouched");
        let history = f.quarantine.history();
        assert_eq!(history[0].purged_files, 1);
        assert_eq!(history[0].purged_bytes, 5);
    }

    #[test]
    fn keeps_a_copy_whose_original_changed_or_vanished() {
        let f = fixture();
        let now = now_unix();
        let (keep, _copy, _plan) = cleaned(&f, "edited", b"v1");
        fs::write(&keep, b"v2!").unwrap();
        let (keep2, _copy2, _plan2) = cleaned(&f, "deleted", b"only");
        fs::remove_file(&keep2).unwrap();

        let outcome = f.quarantine.purge_expired(now + LATER).unwrap();
        assert_eq!(outcome.purged, 0);
        assert_eq!(outcome.kept.len(), 2);
        assert!(outcome
            .kept
            .iter()
            .any(|(_, why)| why.contains("has changed")));
        assert!(outcome.kept.iter().any(|(_, why)| why.contains("is gone")));
        assert_eq!(f.quarantine.waiting().0, 2, "the only copies stay safe");
    }

    #[test]
    fn keeps_a_copy_that_was_tampered_with() {
        let f = fixture();
        let (_keep, _copy, plan) = cleaned(&f, "doc", b"contract");
        let manifest = f.quarantine.read_manifest(&plan.id).unwrap();
        fs::write(&manifest.entries[0].stored, b"something else").unwrap();
        let outcome = f.quarantine.purge_now(now_unix()).unwrap();
        assert_eq!(outcome.purged, 0);
        assert!(outcome.kept[0].1.contains("safe place has changed"));
    }

    #[test]
    fn undo_refuses_when_stored_payload_is_modified() {
        let f = fixture();
        let (_keep, copy, plan) = cleaned(&f, "doc", b"contract");
        let manifest = f.quarantine.read_manifest(&plan.id).unwrap();
        // Tamper with the stored copy in quarantine (A24)
        fs::write(&manifest.entries[0].stored, b"tampered contract").unwrap();
        let outcome = f.quarantine.undo(&plan.id).unwrap();
        assert_eq!(outcome.restored, 0);
        assert_eq!(outcome.skipped.len(), 1);
        assert!(outcome.skipped[0].1.contains("integrity check failed"));
        assert!(!copy.exists(), "tampered payload was not restored");
    }

    #[test]
    fn never_purges_when_the_clock_reads_before_the_clean_up() {
        let f = fixture();
        cleaned(&f, "x", b"data");
        let outcome = f.quarantine.purge_now(1_000).unwrap();
        assert_eq!(outcome.purged, 0);
        assert_eq!(f.quarantine.waiting().0, 1);
    }

    #[test]
    fn never_purges_clean_ups_without_a_record_of_the_kept_copy() {
        let f = fixture();
        let (_keep, _copy, plan) = cleaned(&f, "old", b"legacy");
        let mut manifest = f.quarantine.read_manifest(&plan.id).unwrap();
        manifest.entries[0].keep = None;
        f.quarantine.write_manifest(&manifest).unwrap();
        let outcome = f.quarantine.purge_now(now_unix()).unwrap();
        assert_eq!(outcome.purged, 0);
        assert!(outcome.kept[0].1.contains("no record"));
    }

    #[test]
    fn reads_manifests_written_before_purging_existed() {
        let f = fixture();
        let (_keep, copy, plan) = cleaned(&f, "legacy", b"bytes");
        // The old format: no keep, restored or purge fields.
        let path = f.quarantine.plan_dir(&plan.id).join(MANIFEST);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for field in ["keep", "keep_modified"] {
            value["entries"][0].as_object_mut().unwrap().remove(field);
        }
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(f.quarantine.waiting().0, 1);
        assert_eq!(f.quarantine.undo_last().unwrap().restored, 1);
        assert!(copy.exists());
    }

    #[test]
    fn undo_after_a_purge_recreates_the_copy_from_the_original() {
        let f = fixture();
        let (keep, copy, _plan) = cleaned(&f, "song", b"melody");
        let purged = f.quarantine.purge_now(now_unix()).unwrap();
        assert_eq!(purged.purged, 1);
        assert!(!copy.exists());

        let undo = f.quarantine.undo_last().unwrap();
        assert_eq!(undo.restored, 1);
        assert_eq!(fs::read(&copy).unwrap(), b"melody");
        assert_eq!(fs::read(&keep).unwrap(), b"melody");
        assert!(matches!(
            f.quarantine.undo_last(),
            Err(CleanupError::NothingToUndo)
        ));
    }

    #[test]
    fn undo_after_a_purge_needs_the_original_unchanged() {
        let f = fixture();
        let (keep, copy, _plan) = cleaned(&f, "song", b"melody");
        f.quarantine.purge_now(now_unix()).unwrap();
        fs::write(&keep, b"remixed").unwrap();
        let undo = f.quarantine.undo_last().unwrap();
        assert_eq!(undo.restored, 0);
        assert!(undo.skipped[0].1.contains("already freed"));
        assert!(!copy.exists());
    }

    #[test]
    fn runs_only_the_plan_that_was_approved_and_only_once() {
        use custodian_approval::{ApprovalKey, Verifier};
        let f = fixture();
        let keep = f.root.join("song.mp3");
        let copy = f.root.join("song copy.mp3");
        let other = f.root.join("song (2).mp3");
        for p in [&keep, &copy, &other] {
            fs::write(p, b"tune").unwrap();
        }
        let key = ApprovalKey::generate();
        let verifier = Verifier::new(key.verifying_key());
        let now = now_unix();
        let plan = CleanupPlan::new(vec![item(&keep, &copy)]);
        let approval = key.sign(&plan.digest(), now, 120);

        // The same approval cannot run a different plan, or this plan with a
        // file swapped in after approval.
        let mut swapped = plan.clone();
        swapped.items[0].path = other.clone();
        assert!(matches!(
            f.quarantine
                .execute_approved(&swapped, &approval, &verifier, now),
            Err(CleanupError::NotApproved(ApprovalError::Auth))
        ));
        assert!(other.exists() && copy.exists());

        let outcome = f
            .quarantine
            .execute_approved(&plan, &approval, &verifier, now)
            .unwrap();
        assert_eq!(outcome.moved, 1);
        assert!(!copy.exists());

        fs::write(&copy, b"tune").unwrap();
        assert!(matches!(
            f.quarantine
                .execute_approved(&plan, &approval, &verifier, now),
            Err(CleanupError::NotApproved(ApprovalError::NonceReused))
        ));
        assert!(copy.exists());
    }

    #[test]
    fn plan_digest_covers_every_field() {
        let base = CleanupPlan {
            id: "p".into(),
            items: vec![PlanItem {
                path: "a".into(),
                keep: "b".into(),
                size: 1,
                hash: "h".into(),
            }],
        };
        let digest = base.digest();
        assert_eq!(digest, base.clone().digest());
        let mut changes: Vec<CleanupPlan> = vec![base.clone(); 5];
        changes[0].id = "q".into();
        changes[1].items[0].path = "c".into();
        changes[2].items[0].keep = "c".into();
        changes[3].items[0].size = 2;
        changes[4].items[0].hash = "i".into();
        for changed in changes {
            assert_ne!(changed.digest(), digest);
        }
    }

    #[test]
    fn never_proposes_copies_in_synced_folders_and_keeps_those_instead() {
        use crate::walk::FileEntry;
        let entry = |p: &str| FileEntry {
            path: PathBuf::from(p),
            size: 10,
            modified: None,
        };
        let group = |files: &[&str]| DuplicateGroup {
            size: 10,
            hash: "h".into(),
            files: files.iter().map(|p| entry(p)).collect(),
        };
        let groups = vec![
            // A local copy and a OneDrive copy: the OneDrive one stays, the
            // local one can go.
            group(&["/u/Downloads/a.jpg", "/u/OneDrive/Pictures/a.jpg"]),
            // Both in OneDrive: nothing may be proposed.
            group(&["/u/OneDrive/b.jpg", "/u/OneDrive/Desktop/b.jpg"]),
            // Neither in OneDrive: the first stays, as before.
            group(&["/u/Music/c.mp3", "/u/Downloads/c.mp3"]),
        ];
        let in_onedrive = |p: &Path| p.starts_with("/u/OneDrive").then(|| "OneDrive".to_string());
        let (items, left) = duplicate_plan_items(&groups, &in_onedrive);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].path, PathBuf::from("/u/Downloads/a.jpg"));
        assert_eq!(items[0].keep, PathBuf::from("/u/OneDrive/Pictures/a.jpg"));
        assert_eq!(items[1].path, PathBuf::from("/u/Downloads/c.mp3"));
        assert_eq!(items[1].keep, PathBuf::from("/u/Music/c.mp3"));
        assert!(items.iter().all(|i| !i.path.starts_with("/u/OneDrive")));
        assert_eq!(
            left,
            LeftOut {
                files: 1,
                bytes: 10,
                places: vec!["OneDrive".into()]
            }
        );
    }

    #[test]
    fn restore_names_never_repeat() {
        let names: Vec<PathBuf> = restore_targets(Path::new("C:\\u\\photo.jpg"))
            .take(3)
            .collect();
        assert_eq!(
            names,
            vec![
                PathBuf::from("C:\\u\\photo.jpg"),
                PathBuf::from("C:\\u\\photo (restored).jpg"),
                PathBuf::from("C:\\u\\photo (restored 2).jpg"),
            ]
        );
        let no_ext: Vec<PathBuf> = restore_targets(Path::new("notes"))
            .skip(1)
            .take(1)
            .collect();
        assert_eq!(no_ext, vec![PathBuf::from("notes (restored)")]);
    }

    #[test]
    fn refuses_manifest_with_traversal_or_out_of_vault_path() {
        let f = fixture();
        // Path traversal in plan id is rejected
        assert!(f.quarantine.read_manifest("../etc").is_err());
        assert!(f.quarantine.read_manifest("..\\windows").is_err());
        assert!(f.quarantine.read_manifest("a/b").is_err());

        // Manifest containing stored path outside vault directory is rejected
        let plan_dir = f.quarantine.plan_dir("validplan123");
        fs::create_dir_all(&plan_dir).unwrap();
        let malicious_manifest = serde_json::json!({
            "plan_id": "validplan123",
            "created_unix": 1000,
            "undone": false,
            "entries": [{
                "original": "C:\\safe\\file.txt",
                "stored": "C:\\Windows\\System32\\cmd.exe",
                "size": 10,
                "hash": "abc"
            }]
        });
        fs::write(
            plan_dir.join(MANIFEST),
            serde_json::to_vec(&malicious_manifest).unwrap(),
        )
        .unwrap();

        let err = f.quarantine.read_manifest("validplan123").unwrap_err();
        assert!(err.to_string().contains("outside vault root"), "{err}");
    }

    #[test]
    fn rejects_stored_path_that_escapes_through_parent_components() {
        let f = fixture();
        let (_, _, plan) = cleaned(&f, "doc", b"contract");
        let mut manifest = f.quarantine.read_manifest(&plan.id).unwrap();
        manifest.entries[0].stored = f.quarantine.plan_dir(&plan.id).join("../outside");
        f.quarantine.write_manifest(&manifest).unwrap();
        let err = f.quarantine.read_manifest(&plan.id).unwrap_err();
        assert!(err.to_string().contains("outside vault root"), "{err}");
    }
}
