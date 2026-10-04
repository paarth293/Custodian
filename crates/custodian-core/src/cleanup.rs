//! Safe, undoable clean-up: files are moved into Custodian's quarantine
//! ("the safe place") instead of being deleted, with a manifest that lets
//! every action be reversed and checked by hash (Custodian Contract, D-06).
//!
//! Before moving a duplicate, the copy being kept is re-hashed. A plan can
//! therefore never remove the last copy of a file, even if files changed
//! between the scan and the approval.

use crate::hashing::hash_file;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const MANIFEST: &str = "manifest.json";

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ManifestEntry {
    original: PathBuf,
    stored: PathBuf,
    size: u64,
    hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Manifest {
    plan_id: String,
    created_unix: u64,
    undone: bool,
    entries: Vec<ManifestEntry>,
}

/// Result of executing a plan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CleanupOutcome {
    pub plan_id: String,
    pub moved: usize,
    pub bytes: u64,
    /// Items left untouched, with the reason.
    pub skipped: Vec<(PathBuf, String)>,
}

/// Result of undoing a plan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UndoOutcome {
    pub plan_id: String,
    pub restored: usize,
    pub skipped: Vec<(PathBuf, String)>,
}

/// One past clean-up, for the history view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryEntry {
    pub plan_id: String,
    pub created_unix: u64,
    pub files: usize,
    pub bytes: u64,
    pub undone: bool,
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

/// Moves a file, falling back to copy + verify + delete across volumes.
fn move_verified(from: &Path, to: &Path, hash: &str) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(from, to).is_err() {
        fs::copy(from, to)?;
        if hash_file(to)? != hash {
            let _ = fs::remove_file(to);
            return Err(io::Error::other("copy verification failed"));
        }
        fs::remove_file(from)?;
    }
    Ok(())
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
        fs::write(&tmp, serde_json::to_vec_pretty(manifest)?)?;
        fs::rename(tmp, dir.join(MANIFEST))?;
        Ok(())
    }

    fn read_manifest(&self, plan_id: &str) -> Result<Manifest, CleanupError> {
        let path = self.plan_dir(plan_id).join(MANIFEST);
        let bytes = fs::read(&path).map_err(|_| CleanupError::UnknownPlan(plan_id.to_string()))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn manifests(&self) -> Vec<Manifest> {
        let Ok(dirs) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut all: Vec<Manifest> = dirs
            .flatten()
            .filter_map(|d| fs::read(d.path().join(MANIFEST)).ok())
            .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
            .collect();
        all.sort_by(|a, b| {
            b.created_unix
                .cmp(&a.created_unix)
                .then(b.plan_id.cmp(&a.plan_id))
        });
        all
    }

    /// Moves every item of `plan` into quarantine. Items whose file or kept
    /// copy changed since the scan are skipped, never forced.
    pub fn execute(&self, plan: &CleanupPlan) -> Result<CleanupOutcome, CleanupError> {
        let mut manifest = Manifest {
            plan_id: plan.id.clone(),
            created_unix: now_unix(),
            undone: false,
            entries: Vec::new(),
        };
        let mut outcome = CleanupOutcome {
            plan_id: plan.id.clone(),
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
            let name = item
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let stored = files_dir.join(format!("{index:05}_{name}"));
            if let Err(err) = move_verified(&item.path, &stored, &item.hash) {
                outcome.skipped.push(skip(&err.to_string()));
                continue;
            }
            manifest.entries.push(ManifestEntry {
                original: item.path.clone(),
                stored,
                size: item.size,
                hash: item.hash.clone(),
            });
            // Persist after every move so a crash never loses track of a file.
            self.write_manifest(&manifest)?;
            outcome.moved += 1;
            outcome.bytes += item.size;
        }
        if !manifest.entries.is_empty() {
            self.write_manifest(&manifest)?;
        }
        Ok(outcome)
    }

    /// Puts every file of a past clean-up back where it was, verified by hash.
    pub fn undo(&self, plan_id: &str) -> Result<UndoOutcome, CleanupError> {
        let mut manifest = self.read_manifest(plan_id)?;
        let mut outcome = UndoOutcome {
            plan_id: plan_id.to_string(),
            ..Default::default()
        };
        if manifest.undone {
            return Ok(outcome);
        }
        for entry in &manifest.entries {
            if entry.original.exists() {
                outcome.skipped.push((
                    entry.original.clone(),
                    "a file with that name exists again".into(),
                ));
                continue;
            }
            match move_verified(&entry.stored, &entry.original, &entry.hash) {
                Ok(()) => outcome.restored += 1,
                Err(err) => outcome
                    .skipped
                    .push((entry.original.clone(), err.to_string())),
            }
        }
        manifest.undone = true;
        self.write_manifest(&manifest)?;
        Ok(outcome)
    }

    /// Undoes the most recent clean-up that is not undone yet.
    pub fn undo_last(&self) -> Result<UndoOutcome, CleanupError> {
        let last = self
            .manifests()
            .into_iter()
            .find(|m| !m.undone)
            .ok_or(CleanupError::NothingToUndo)?;
        self.undo(&last.plan_id)
    }

    /// Past clean-ups, newest first.
    pub fn history(&self) -> Vec<HistoryEntry> {
        self.manifests()
            .into_iter()
            .map(|m| HistoryEntry {
                files: m.entries.len(),
                bytes: m.entries.iter().map(|e| e.size).sum(),
                plan_id: m.plan_id,
                created_unix: m.created_unix,
                undone: m.undone,
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
        assert!(keep.exists());
        assert!(!copy.exists());
        assert_eq!(f.quarantine.history()[0].files, 1);

        let undo = f.quarantine.undo_last().unwrap();
        assert_eq!(undo.restored, 1);
        assert_eq!(fs::read(&copy).unwrap(), b"picture");
        assert!(f.quarantine.history()[0].undone);
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
    fn undo_does_not_overwrite_a_new_file() {
        let f = fixture();
        let keep = f.root.join("a.txt");
        let copy = f.root.join("b.txt");
        fs::write(&keep, b"same").unwrap();
        fs::write(&copy, b"same").unwrap();
        let plan = CleanupPlan::new(vec![item(&keep, &copy)]);
        f.quarantine.execute(&plan).unwrap();

        fs::write(&copy, b"brand new file").unwrap();
        let undo = f.quarantine.undo(&plan.id).unwrap();
        assert_eq!(undo.restored, 0);
        assert_eq!(fs::read(&copy).unwrap(), b"brand new file");
    }
}
