//! Standing permission for one native, bounded workflow. No model may edit it.
use crate::file_tasks::{CopyLimits, FileAction, FileReceipt, FileTaskPlan, FileTasks};
use crate::walk::{walk_files, FileEntry, WalkLimits, WalkStats};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Mutex,
};

pub const MAX_BACKUP_BYTES: u64 = 100 * 1024 * 1024;
pub const FREE_RESERVE: u64 = 1024 * 1024 * 1024;
const MAX_POLICY_BYTES: usize = 128 * 1024;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineSettings {
    pub revision: u64,
    pub automatic_backup: bool,
    #[serde(default)]
    pub automatic_collections: bool,
    /// Empty means any explicitly requested permitted local location, matching
    /// the original opt-in grant. A populated list narrows it by directory.
    #[serde(default)]
    pub source_roots: Vec<PathBuf>,
    #[serde(default)]
    pub destination_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyOperation {
    LargestBackup,
    Collection,
}

#[derive(Debug, Default)]
pub struct RoutinePolicy {
    settings: Mutex<RoutineSettings>,
    revision: AtomicU64,
    path: Option<PathBuf>,
}

fn denied(reason: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, reason)
}

impl RoutinePolicy {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let settings = match fs::read(&path) {
            Ok(bytes) if bytes.len() <= MAX_POLICY_BYTES => {
                serde_json::from_slice(&custodian_winsec::dpapi::unprotect(&bytes)?)
                    .map_err(io::Error::other)?
            }
            Ok(_) => return Err(denied("invalid standing permission")),
            Err(e)
                if e.kind() == io::ErrorKind::NotFound
                    && !path.with_extension("required").exists() =>
            {
                RoutineSettings::default()
            }
            Err(e) => return Err(e),
        };
        Ok(Self {
            revision: AtomicU64::new(settings.revision),
            settings: Mutex::new(settings),
            path: Some(path),
        })
    }

    pub fn settings(&self) -> RoutineSettings {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Capturing a request must not block unrelated chat behind a running copy.
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    pub fn update(&self, revision: u64, automatic_backup: bool) -> io::Result<RoutineSettings> {
        self.update_permissions(revision, automatic_backup, None)
    }

    pub fn update_permissions(
        &self,
        revision: u64,
        automatic_backup: bool,
        automatic_collections: Option<bool>,
    ) -> io::Result<RoutineSettings> {
        self.update_scopes(
            revision,
            automatic_backup,
            automatic_collections,
            None,
            None,
        )
    }

    pub fn update_scopes(
        &self,
        revision: u64,
        automatic_backup: bool,
        automatic_collections: Option<bool>,
        source_roots: Option<Vec<PathBuf>>,
        destination_roots: Option<Vec<PathBuf>>,
    ) -> io::Result<RoutineSettings> {
        let mut settings = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        if settings.revision != revision {
            return Err(denied("permission changed; reload preferences"));
        }
        let next = RoutineSettings {
            revision: revision
                .checked_add(1)
                .ok_or_else(|| denied("invalid revision"))?,
            automatic_backup,
            automatic_collections: automatic_collections.unwrap_or(settings.automatic_collections),
            source_roots: source_roots
                .filter(|roots| roots != &settings.source_roots)
                .map(validate_roots)
                .transpose()?
                .unwrap_or_else(|| settings.source_roots.clone()),
            destination_roots: destination_roots
                .filter(|roots| roots != &settings.destination_roots)
                .map(validate_roots)
                .transpose()?
                .unwrap_or_else(|| settings.destination_roots.clone()),
        };
        if let Some(path) = &self.path {
            fs::create_dir_all(
                path.parent()
                    .ok_or_else(|| denied("missing settings folder"))?,
            )?;
            let sealed = custodian_winsec::dpapi::protect(
                &serde_json::to_vec(&next).map_err(io::Error::other)?,
            )?;
            if sealed.len() > MAX_POLICY_BYTES {
                return Err(denied("Permission scopes are too large to persist safely"));
            }
            let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&sealed)?;
            file.sync_all()?;
            drop(file);
            let marker = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(path.with_extension("required"))?;
            marker.sync_all()?;
            fs::rename(temporary, path)?;
        }
        *settings = next.clone();
        self.revision.store(next.revision, Ordering::Release);
        Ok(next)
    }

    /// A standing grant authorizes only creation of verified, non-overwriting
    /// copies. It never authorizes moves, settings, app control or deletion.
    /// Validate resources both before discovery and inside the commit lock.
    pub fn with_copy_grant<T>(
        &self,
        operation: CopyOperation,
        revision: u64,
        sources: &[PathBuf],
        destination: &Path,
        run: impl FnOnce() -> io::Result<T>,
    ) -> io::Result<T> {
        let settings = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        let enabled = match operation {
            CopyOperation::LargestBackup => settings.automatic_backup,
            CopyOperation::Collection => settings.automatic_collections,
        };
        if !enabled || settings.revision != revision || sources.is_empty() {
            return Err(denied(
                "Copy permission is off or changed; ask again with a current grant",
            ));
        }
        for source in sources {
            check_scope(source, &settings.source_roots)?;
        }
        check_scope(destination, &settings.destination_roots)?;
        run()
    }

    /// Serializes permission revocation with the bounded mutation. Revocation
    /// invalidates a task still scanning; an in-flight copy finishes safely.
    pub fn with_backup<T>(
        &self,
        revision: u64,
        run: impl FnOnce() -> io::Result<T>,
    ) -> io::Result<T> {
        let settings = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        if !settings.automatic_backup || settings.revision != revision {
            return Err(denied("Automatic backup is off or its permission changed. Enable it in Routine permissions and ask again."));
        }
        run()
    }

    pub fn with_collection<T>(
        &self,
        revision: u64,
        run: impl FnOnce() -> io::Result<T>,
    ) -> io::Result<T> {
        let settings = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        if !settings.automatic_collections || settings.revision != revision {
            return Err(denied("Automatic collections are off or their permission changed. Enable them in Routine permissions and ask again."));
        }
        run()
    }
}

fn validate_roots(roots: Vec<PathBuf>) -> io::Result<Vec<PathBuf>> {
    if roots.len() > 16 {
        return Err(denied("Use at most 16 folders per scope"));
    }
    let mut resolved = Vec::new();
    for root in roots {
        if root.as_os_str().len() > 2048 {
            return Err(denied("A permission folder path is too long"));
        }
        crate::privacy::check(&root)?;
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(denied("Grant scopes must be existing local folders"));
        }
        crate::privacy::check(&root)?;
        if !resolved.contains(&root) {
            resolved.push(root);
        }
    }
    Ok(resolved)
}

fn check_scope(path: &Path, roots: &[PathBuf]) -> io::Result<()> {
    crate::privacy::check(path)?;
    let resolved = if path.exists() {
        path.canonicalize()?
    } else {
        path.parent()
            .ok_or_else(|| denied("Missing resource parent"))?
            .canonicalize()?
            .join(
                path.file_name()
                    .ok_or_else(|| denied("Missing resource name"))?,
            )
    };
    crate::privacy::check(&resolved)?;
    if !roots.is_empty() && !roots.iter().any(|root| resolved.starts_with(root)) {
        return Err(denied(
            "This location is outside the folders allowed in Routine permissions",
        ));
    }
    Ok(())
}

pub struct BackupPlan {
    pub plan: FileTaskPlan,
    pub stats: WalkStats,
    pub protected_files: usize,
    revision: u64,
}

/// Capture a native user goal: find, select and prepare one exact ordinary file.
/// `eligible` is the trusted host's protected-path predicate, never model output.
pub fn prepare_backup(
    policy: &RoutinePolicy,
    revision: u64,
    roots: &[PathBuf],
    destination: &Path,
    cancel: &AtomicBool,
    eligible: &dyn Fn(&Path) -> bool,
    progress: &mut dyn FnMut(usize),
) -> io::Result<BackupPlan> {
    policy.with_copy_grant(
        CopyOperation::LargestBackup,
        revision,
        roots,
        destination,
        || Ok(()),
    )?;
    let settings = policy.settings();
    if !settings.automatic_backup || settings.revision != revision {
        return Err(denied(
            "Automatic backup is off. Enable it in Routine permissions and ask again.",
        ));
    }
    crate::privacy::check(destination)?;
    let destination = destination.canonicalize()?;
    crate::privacy::check(&destination)?;
    if !destination.is_dir() || !eligible(&destination) {
        return Err(denied("That backup destination is not permitted"));
    }
    if roots.is_empty() {
        return Err(denied("No permitted source was provided"));
    }
    let mut resolved = Vec::new();
    for root in roots {
        crate::privacy::check(root)?;
        let root = root.canonicalize()?;
        crate::privacy::check(&root)?;
        if !root.is_dir() || root == destination {
            return Err(denied("Choose distinct source and destination folders"));
        }
        resolved.push(root);
    }
    let exclude_destination = resolved.iter().any(|r| destination.starts_with(r));
    let mut selected: Option<FileEntry> = None;
    let mut protected_files = 0;
    let stats = walk_files(
        &resolved,
        WalkLimits::whole_system_read(),
        cancel,
        progress,
        &mut |entry| {
            if !eligible(&entry.path)
                || (exclude_destination && entry.path.starts_with(&destination))
            {
                protected_files += 1;
                return;
            }
            if selected.as_ref().is_none_or(|old| {
                entry.size > old.size || (entry.size == old.size && entry.path < old.path)
            }) {
                selected = Some(entry);
            }
        },
    );
    if stats.truncated || cancel.load(Ordering::Relaxed) || !crate::privacy::request_is_current() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Backup stopped before copying",
        ));
    }
    let selected = selected.ok_or_else(|| denied("No permitted ordinary file was found"))?;
    if selected.size > MAX_BACKUP_BYTES {
        return Err(denied("The largest permitted file exceeds the 100 MiB automatic-backup limit. Nothing was copied."));
    }
    let target = destination.join(
        selected
            .path
            .file_name()
            .ok_or_else(|| denied("missing file name"))?,
    );
    let plan = FileTaskPlan::prepare(FileAction::Copy, Some(&selected.path), &target)?;
    if plan.bytes() != selected.size
        || !eligible(plan.source.as_ref().unwrap())
        || !eligible(&plan.destination)
    {
        return Err(denied("The selected file changed or is not permitted"));
    }
    Ok(BackupPlan {
        plan,
        stats,
        protected_files,
        revision,
    })
}

pub fn execute_backup(
    policy: &RoutinePolicy,
    prepared: &BackupPlan,
    store: &FileTasks,
    cancel: &AtomicBool,
) -> io::Result<FileReceipt> {
    policy.with_copy_grant(
        CopyOperation::LargestBackup,
        prepared.revision,
        std::slice::from_ref(
            prepared
                .plan
                .source
                .as_ref()
                .ok_or_else(|| denied("Missing copy source"))?,
        ),
        &prepared.plan.destination,
        || {
            if cancel.load(Ordering::Relaxed) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Backup stopped before copying",
                ));
            }
            store.execute_bounded_copy(
                &prepared.plan,
                CopyLimits {
                    max_bytes: MAX_BACKUP_BYTES,
                    min_free_bytes: FREE_RESERVE,
                    max_space_percent: 5,
                },
                cancel,
            )
        },
    )
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn scoped_grants_bind_operation_resources_and_revision_before_discovery_and_commit() {
        let (root, source, destination, files) = fixture();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("other.txt"), b"outside canary").unwrap();
        let path = root.path().join("policy.dpapi");
        let policy = RoutinePolicy::open(path.clone()).unwrap();
        policy
            .update_scopes(
                0,
                true,
                Some(false),
                Some(vec![source.clone()]),
                Some(vec![destination.clone()]),
            )
            .unwrap();
        let cancel = AtomicBool::new(false);
        let prepare = |roots: &[PathBuf], target: &Path| {
            prepare_backup(
                &policy,
                policy.revision(),
                roots,
                target,
                &cancel,
                &|_| true,
                &mut |_| panic!("out-of-scope discovery"),
            )
        };
        assert!(prepare(std::slice::from_ref(&outside), &destination).is_err());
        assert!(prepare(std::slice::from_ref(&source), &outside).is_err());
        assert!(files.receipts().unwrap().is_empty());
        assert!(policy
            .with_copy_grant(
                CopyOperation::Collection,
                1,
                std::slice::from_ref(&source),
                &destination,
                || -> io::Result<()> { panic!("backup grant expanded to collections") }
            )
            .is_err());
        let prepared = prepare_backup(
            &policy,
            1,
            std::slice::from_ref(&source),
            &destination,
            &cancel,
            &|_| true,
            &mut |_| {},
        )
        .unwrap();
        policy
            .update_scopes(1, true, Some(false), None, Some(vec![outside.clone()]))
            .unwrap();
        assert!(execute_backup(&policy, &prepared, &files, &cancel).is_err());
        assert!(!prepared.plan.destination.exists());
        let reopened = RoutinePolicy::open(path).unwrap();
        assert_eq!(
            reopened.settings().source_roots,
            vec![source.canonicalize().unwrap()]
        );
        assert_eq!(
            reopened.settings().destination_roots,
            vec![outside.canonicalize().unwrap()]
        );
        assert!(reopened
            .with_copy_grant(CopyOperation::LargestBackup, 1, &[source], &outside, || Ok(
                ()
            ))
            .is_err());
    }

    #[test]
    fn old_single_file_grant_never_enables_collection_permission_on_upgrade() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("routine.dpapi");
        fs::write(
            &path,
            custodian_winsec::dpapi::protect(br#"{"revision":11,"automaticBackup":true}"#).unwrap(),
        )
        .unwrap();
        let policy = RoutinePolicy::open(path).unwrap();
        assert_eq!(policy.revision(), 11);
        assert!(policy.settings().automatic_backup);
        assert!(!policy.settings().automatic_collections);
        assert!(policy.with_collection(11, || Ok(())).is_err());
        policy.update_permissions(11, true, Some(true)).unwrap();
        assert!(policy.with_collection(11, || Ok(())).is_err());
        assert!(policy.with_collection(12, || Ok(())).is_ok());
    }

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, FileTasks) {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("project");
        let dest = root.path().join("backup");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&dest).unwrap();
        fs::write(source.join("report.txt"), b"largest ordinary document").unwrap();
        fs::write(source.join("small.txt"), b"tiny").unwrap();
        let store = FileTasks::new(root.path().join("receipts"));
        (root, source, dest, store)
    }

    #[test]
    fn permission_defaults_off_persists_and_refuses_stale_or_tampered_updates() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("routine.dpapi");
        let policy = RoutinePolicy::open(path.clone()).unwrap();
        assert!(!policy.settings().automatic_backup);
        assert!(policy.with_backup(0, || Ok(())).is_err());
        policy.update(0, true).unwrap();
        assert!(policy.update(0, false).is_err());
        assert!(
            RoutinePolicy::open(path.clone())
                .unwrap()
                .settings()
                .automatic_backup
        );
        fs::write(&path, b"tampered").unwrap();
        assert!(RoutinePolicy::open(path.clone()).is_err());
        fs::remove_file(path.clone()).unwrap();
        assert!(RoutinePolicy::open(path).is_err());
    }

    #[test]
    fn revocation_waits_for_mutation_without_blocking_request_revision_capture() {
        use std::{sync::mpsc, thread, time::Duration};
        let policy = Arc::new(RoutinePolicy::default());
        policy.update(0, true).unwrap();
        let (entered, ready) = mpsc::channel();
        let (release, finish) = mpsc::channel();
        let running = policy.clone();
        let worker = thread::spawn(move || {
            running.with_backup(1, || {
                entered.send(()).unwrap();
                finish.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(())
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(policy.revision(), 1);
        let (revoked, done) = mpsc::channel();
        let changing = policy.clone();
        let updater = thread::spawn(move || revoked.send(changing.update(1, false)).unwrap());
        assert!(done.recv_timeout(Duration::from_millis(50)).is_err());
        release.send(()).unwrap();
        assert!(
            !done
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap()
                .automatic_backup
        );
        worker.join().unwrap().unwrap();
        updater.join().unwrap();
        assert_eq!(policy.revision(), 2);
        assert!(policy.with_backup(1, || Ok(())).is_err());
    }

    #[test]
    fn native_backup_excludes_private_largest_preserves_original_and_undo_after_restart() {
        let (root, source, dest, store) = fixture();
        let private = source.join("private");
        fs::create_dir(&private).unwrap();
        fs::write(private.join("private-canary.txt"), vec![7; 4096]).unwrap();
        let privacy = Arc::new(crate::privacy::AccessPolicy::default());
        privacy.replace_private_paths(vec![private]).unwrap();
        let policy = RoutinePolicy::default();
        policy.update(0, true).unwrap();
        crate::privacy::with_policy(privacy, || {
            let cancel = AtomicBool::new(false);
            let prepared = prepare_backup(
                &policy,
                policy.settings().revision,
                std::slice::from_ref(&source),
                &dest,
                &cancel,
                &|_| true,
                &mut |_| {},
            )
            .unwrap();
            assert!(prepared.stats.excluded_count > 0);
            assert_eq!(
                prepared.plan.source.as_ref().unwrap().file_name().unwrap(),
                "report.txt"
            );
            let receipt = execute_backup(&policy, &prepared, &store, &cancel).unwrap();
            assert_eq!(
                fs::read(&receipt.destination).unwrap(),
                b"largest ordinary document"
            );
            assert_eq!(
                fs::read(source.join("report.txt")).unwrap(),
                b"largest ordinary document"
            );
            assert!(execute_backup(&policy, &prepared, &store, &cancel).is_err());
            FileTasks::new(root.path().join("receipts"))
                .undo(&receipt.id)
                .unwrap();
            assert!(!receipt.destination.exists());
            assert!(source.join("report.txt").exists());
        });
    }

    #[test]
    fn revoked_permission_cancelled_and_changed_source_cannot_start_copy() {
        let (_root, source, dest, store) = fixture();
        let policy = RoutinePolicy::default();
        policy.update(0, true).unwrap();
        let cancel = AtomicBool::new(false);
        let prepared = prepare_backup(
            &policy,
            policy.settings().revision,
            std::slice::from_ref(&source),
            &dest,
            &cancel,
            &|_| true,
            &mut |_| {},
        )
        .unwrap();
        policy.update(1, false).unwrap();
        policy.update(2, true).unwrap();
        assert!(prepare_backup(
            &policy,
            1,
            std::slice::from_ref(&source),
            &dest,
            &cancel,
            &|_| true,
            &mut |_| {},
        )
        .is_err());
        assert!(execute_backup(&policy, &prepared, &store, &cancel).is_err());
        assert!(!dest.join("report.txt").exists());
        let prepared = prepare_backup(
            &policy,
            policy.settings().revision,
            std::slice::from_ref(&source),
            &dest,
            &cancel,
            &|_| true,
            &mut |_| {},
        )
        .unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert!(execute_backup(&policy, &prepared, &store, &cancel).is_err());
        assert!(prepare_backup(
            &policy,
            policy.settings().revision,
            std::slice::from_ref(&source),
            &dest,
            &cancel,
            &|_| true,
            &mut |_| {}
        )
        .is_err());
        cancel.store(false, Ordering::Relaxed);
        fs::write(source.join("report.txt"), b"changed").unwrap();
        assert!(execute_backup(&policy, &prepared, &store, &cancel).is_err());
        assert!(!dest.join("report.txt").exists());
    }

    #[test]
    fn size_capacity_and_existing_destination_limits_are_enforced() {
        let (_root, source, dest, store) = fixture();
        let policy = RoutinePolicy::default();
        policy.update(0, true).unwrap();
        let cancel = AtomicBool::new(false);
        let prepared = prepare_backup(
            &policy,
            policy.settings().revision,
            std::slice::from_ref(&source),
            &dest,
            &cancel,
            &|_| true,
            &mut |_| {},
        )
        .unwrap();
        assert!(store
            .execute_bounded_copy(
                &prepared.plan,
                CopyLimits {
                    max_bytes: 1,
                    min_free_bytes: 0,
                    max_space_percent: 5
                },
                &cancel
            )
            .is_err());
        assert!(store
            .execute_bounded_copy(
                &prepared.plan,
                CopyLimits {
                    max_bytes: MAX_BACKUP_BYTES,
                    min_free_bytes: u64::MAX,
                    max_space_percent: 5
                },
                &cancel
            )
            .is_err());
        assert!(!dest.join("report.txt").exists());
        let huge = fs::File::create(source.join("huge.bin")).unwrap();
        huge.set_len(MAX_BACKUP_BYTES + 1).unwrap();
        drop(huge);
        assert!(prepare_backup(
            &policy,
            policy.settings().revision,
            std::slice::from_ref(&source),
            &dest,
            &cancel,
            &|_| true,
            &mut |_| {}
        )
        .is_err());
        fs::remove_file(source.join("huge.bin")).unwrap();
        fs::write(dest.join("report.txt"), b"existing destination").unwrap();
        assert!(execute_backup(&policy, &prepared, &store, &cancel).is_err());
        assert_eq!(
            fs::read(dest.join("report.txt")).unwrap(),
            b"existing destination"
        );
    }

    #[test]
    fn private_destination_and_protected_candidates_do_not_get_copied() {
        let (_root, source, dest, store) = fixture();
        let policy = RoutinePolicy::default();
        policy.update(0, true).unwrap();
        let cancel = AtomicBool::new(false);
        assert!(prepare_backup(
            &policy,
            policy.settings().revision,
            std::slice::from_ref(&source),
            &dest,
            &cancel,
            &|_| false,
            &mut |_| {}
        )
        .is_err());
        let privacy = Arc::new(crate::privacy::AccessPolicy::default());
        let prepared = crate::privacy::with_policy(privacy.clone(), || {
            prepare_backup(
                &policy,
                policy.settings().revision,
                std::slice::from_ref(&source),
                &dest,
                &cancel,
                &|_| true,
                &mut |_| {},
            )
        })
        .unwrap();
        privacy.replace_private_paths(vec![dest.clone()]).unwrap();
        crate::privacy::with_policy(privacy, || {
            assert!(execute_backup(&policy, &prepared, &store, &cancel).is_err())
        });
        assert!(!dest.join("report.txt").exists());
    }
}
