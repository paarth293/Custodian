//! Filesystem privacy authority. Model output cannot edit this policy.
//! Bind the live policy at each trusted worker entry point with `with_policy`.
use custodian_winsec::fsops::{file_identity, FileIdentity};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, RwLock};

pub const DENIED: &str = "Access is blocked by your privacy rules. Nothing was changed.";
pub const OFFLINE: &str =
    "Online actions are unavailable in this offline release. Nothing was opened or sent.";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Zone {
    path: PathBuf,
    canonical: PathBuf,
    identity: FileIdentity,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Rules {
    revision: u64,
    zones: Vec<Zone>,
}

#[derive(Debug, Default)]
pub struct AccessPolicy {
    rules: RwLock<Rules>,
    // A change waits for a noninterruptible file mutation; new mutations then
    // see the new rules. This is distinct from the rules read lock.
    operation: Mutex<()>,
    storage: Option<PathBuf>,
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, DENIED)
}

fn key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    text.strip_prefix("//?/")
        .unwrap_or(&text)
        .trim_end_matches('/')
        .to_lowercase()
}

fn contains(root: &Path, candidate: &Path) -> bool {
    let root = key(root);
    let candidate = key(candidate);
    candidate == root || candidate.starts_with(&(root + "/"))
}

fn secret_component(path: &Path) -> bool {
    if key(path).contains("/windows/system32/config") {
        return true;
    }
    path.components().any(|part| {
        let Component::Normal(name) = part else {
            return false;
        };
        let name = name
            .to_string_lossy()
            .trim_end_matches(['.', ' '])
            .to_lowercase();
        matches!(
            name.as_str(),
            ".ssh"
                | ".aws"
                | ".azure"
                | ".gnupg"
                | ".kube"
                | ".git"
                | "credentials"
                | "vault"
                | "protect"
                | "tokenbroker"
                | "identitycache"
                | "oneauth"
                | "user data"
                | "login data"
                | "cookies"
                | "logins.json"
                | "key4.db"
                | "secrets.json"
                | "id_rsa"
                | "id_ed25519"
                | "credentials.json"
                | "ntuser.dat"
                | "usrclass.dat"
                | ".env"
        ) || name.starts_with(".env.")
            || name.ends_with(".pem")
            || name.ends_with(".key")
            || name.ends_with(".kdbx")
    })
}

fn valid_local_path(path: &Path) -> bool {
    path.is_absolute()
        && custodian_winsec::fsops::is_local_volume(path)
        && !path.components().any(|part| match part {
            Component::ParentDir | Component::CurDir => true,
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                name.contains(':')
                    || name.ends_with(['.', ' '])
                    || name.chars().any(char::is_control)
            }
            _ => false,
        })
}

fn reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & (0x400 | 0x1000 | 0x0040_0000) != 0
    }
    #[cfg(not(windows))]
    false
}

impl AccessPolicy {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let rules = match fs::read(&path) {
            Ok(sealed) => {
                if sealed.len() > 1024 * 1024 {
                    return Err(invalid());
                }
                let bytes = custodian_winsec::dpapi::unprotect(&sealed)?;
                let rules: Rules = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
                if rules.zones.len() > 256
                    || rules.zones.iter().any(|zone| {
                        !valid_local_path(&zone.path) || !valid_local_path(&zone.canonical)
                    })
                {
                    return Err(invalid());
                }
                rules
            }
            Err(err)
                if err.kind() == io::ErrorKind::NotFound
                    && !path.with_extension("required").exists() =>
            {
                Rules::default()
            }
            Err(_) => return Err(invalid()),
        };
        Ok(Self {
            rules: RwLock::new(rules),
            storage: Some(path),
            ..Self::default()
        })
    }

    pub fn revision(&self) -> u64 {
        self.rules
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .revision
    }

    pub fn private_paths(&self) -> Vec<PathBuf> {
        self.rules
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .zones
            .iter()
            .map(|z| z.path.clone())
            .collect()
    }

    pub fn settings(&self) -> (u64, Vec<PathBuf>) {
        let rules = self.rules.read().unwrap_or_else(|e| e.into_inner());
        (
            rules.revision,
            rules.zones.iter().map(|zone| zone.path.clone()).collect(),
        )
    }

    /// Only the trusted privacy UI calls this, never the model's tool vocabulary.
    /// Paths are validated and persisted before the live rules change.
    pub fn replace_private_paths(&self, paths: Vec<PathBuf>) -> io::Result<()> {
        self.replace_paths(paths, None)
    }

    pub fn replace_if_revision(&self, paths: Vec<PathBuf>, revision: u64) -> io::Result<()> {
        self.replace_paths(paths, Some(revision))
    }

    fn replace_paths(&self, paths: Vec<PathBuf>, expected: Option<u64>) -> io::Result<()> {
        if paths.len() > 256 {
            return Err(invalid());
        }
        let _operation = self.operation.lock().unwrap_or_else(|e| e.into_inner());
        if expected.is_some_and(|revision| revision != self.revision()) {
            return Err(invalid());
        }
        let mut zones = Vec::new();
        for path in paths {
            if !valid_local_path(&path) {
                return Err(invalid());
            }
            let canonical = path.canonicalize().map_err(|_| invalid())?;
            if !valid_local_path(&canonical) {
                return Err(invalid());
            }
            let identity = file_identity(&canonical).map_err(|_| invalid())?;
            if !zones.iter().any(|z: &Zone| key(&z.path) == key(&path)) {
                zones.push(Zone {
                    path,
                    canonical,
                    identity,
                });
            }
        }
        let mut rules = self.rules.write().unwrap_or_else(|e| e.into_inner());
        let replacement = Rules {
            revision: rules.revision.checked_add(1).ok_or_else(invalid)?,
            zones,
        };
        if let Some(path) = &self.storage {
            let parent = path.parent().ok_or_else(invalid)?;
            fs::create_dir_all(parent)?;
            let bytes = serde_json::to_vec(&replacement).map_err(io::Error::other)?;
            let sealed = custodian_winsec::dpapi::protect(&bytes)?;
            let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&sealed)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)?;
            let marker = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(path.with_extension("required"))?;
            marker.sync_all()?;
        }
        *rules = replacement;
        Ok(())
    }

    pub fn allows(&self, path: &Path) -> bool {
        if !valid_local_path(path) || secret_component(path) {
            return false;
        }
        if self
            .storage
            .as_ref()
            .and_then(|path| path.parent())
            .is_some_and(|root| contains(root, path))
        {
            return false;
        }
        let rules = self.rules.read().unwrap_or_else(|e| e.into_inner());
        // This lexical check happens before any filesystem query into a private tree.
        if rules
            .zones
            .iter()
            .any(|z| contains(&z.path, path) || contains(&z.canonical, path))
        {
            return false;
        }
        // Resolve existing ancestors only. Never follow links/cloud placeholders.
        // New destinations are checked against their nearest existing parent.
        let mut existing = None;
        for ancestor in path.ancestors() {
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) => {
                    if reparse(&metadata) {
                        return false;
                    }
                    if !rules.zones.is_empty() {
                        let Ok(identity) = file_identity(ancestor) else {
                            return false;
                        };
                        if rules.zones.iter().any(|z| z.identity == identity) {
                            return false;
                        }
                    }
                    if existing.is_none() {
                        existing = Some((ancestor, metadata));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(_) => return false,
            }
        }
        let Some((ancestor, metadata)) = existing else {
            return false;
        };
        let Ok(canonical) = ancestor.canonicalize() else {
            return false;
        };
        if secret_component(&canonical)
            || rules
                .zones
                .iter()
                .any(|z| contains(&z.canonical, &canonical))
        {
            return false;
        }
        if metadata.is_file() {
            // Hard links can alias a secret or private file under an innocent name.
            // Conservatively exclude multi-linked files rather than reading them.
            return custodian_winsec::fsops::file_link_count(ancestor)
                .is_ok_and(|links| links == 1);
        }
        true
    }

    /// Text already acquired under an earlier policy must not be projected again.
    pub fn allows_text(&self, text: &str) -> bool {
        let text = text.replace('\\', "/").to_lowercase();
        let text = text
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        let rules = self.rules.read().unwrap_or_else(|e| e.into_inner());
        !rules.zones.iter().any(|zone| {
            [key(&zone.path), key(&zone.canonical)].iter().any(|path| {
                let path = path.trim_start_matches('/');
                text.match_indices(path).any(|(index, _)| {
                    text[index + path.len()..]
                        .chars()
                        .next()
                        .is_none_or(|next| {
                            next == '/'
                                || next.is_whitespace()
                                || matches!(next, '"' | '\'' | ')' | ']' | '}' | ',' | ';')
                        })
                })
            })
        })
    }

    pub fn with_revision<T>(&self, revision: u64, work: impl FnOnce() -> T) -> Option<T> {
        let rules = self.rules.read().unwrap_or_else(|e| e.into_inner());
        (rules.revision == revision).then(work)
    }
}

#[derive(Clone)]
struct Binding {
    policy: Arc<AccessPolicy>,
    revision: u64,
}
thread_local! { static ACTIVE: RefCell<Option<Binding>> = const { RefCell::new(None) }; }
static DEFAULT: LazyLock<Arc<AccessPolicy>> = LazyLock::new(|| Arc::new(AccessPolicy::default()));

pub fn current() -> Arc<AccessPolicy> {
    ACTIVE
        .with(|active| {
            active
                .borrow()
                .as_ref()
                .map(|binding| binding.policy.clone())
        })
        .unwrap_or_else(|| DEFAULT.clone())
}

pub fn with_policy<T>(policy: Arc<AccessPolicy>, work: impl FnOnce() -> T) -> T {
    struct Reset(Option<Binding>);
    impl Drop for Reset {
        fn drop(&mut self) {
            ACTIVE.with(|active| *active.borrow_mut() = self.0.take());
        }
    }
    let old = ACTIVE.with(|active| {
        let revision = active
            .borrow()
            .as_ref()
            .filter(|binding| Arc::ptr_eq(&binding.policy, &policy))
            .map_or_else(|| policy.revision(), |binding| binding.revision);
        active.replace(Some(Binding { policy, revision }))
    });
    let _reset = Reset(old);
    work()
}

pub fn check(path: &Path) -> io::Result<()> {
    if request_is_current() && current().allows(path) {
        Ok(())
    } else {
        Err(invalid())
    }
}

pub fn request_is_current() -> bool {
    ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .is_none_or(|binding| binding.revision == binding.policy.revision())
    })
}

pub fn with_mutation<T>(work: impl FnOnce() -> T) -> T {
    let policy = current();
    let _guard = policy.operation.lock().unwrap_or_else(|e| e.into_inner());
    work()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_local_scope_excludes_private_metadata_and_default_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let private = dir.path().join("Private");
        let public = dir.path().join("Private2");
        fs::create_dir_all(&private).unwrap();
        fs::create_dir_all(&public).unwrap();
        fs::write(private.join("canary.txt"), "secret").unwrap();
        fs::write(public.join("report.txt"), "public").unwrap();
        let policy = AccessPolicy::default();
        policy.replace_private_paths(vec![private.clone()]).unwrap();
        assert!(!policy.allows(&private));
        assert!(!policy.allows(&private.join("canary.txt")));
        assert!(policy.allows(&public.join("report.txt")));
        assert!(policy.allows_text(&format!("public: {}", public.join("report.txt").display())));
        assert!(!policy.allows(&dir.path().join(".ssh/id_rsa")));
        assert!(!policy.allows(&dir.path().join(".env")));
        assert!(!policy.allows(&dir.path().join("a/../Private/canary.txt")));
        assert!(!policy.allows_text(&format!("old result: {}", private.display())));
    }

    #[test]
    fn aliases_and_renamed_private_folders_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let private = dir.path().join("private");
        fs::create_dir(&private).unwrap();
        let file = private.join("canary");
        fs::write(&file, "secret").unwrap();
        let policy = AccessPolicy::default();
        policy.replace_private_paths(vec![private.clone()]).unwrap();
        let alias = dir.path().join("innocent");
        fs::hard_link(&file, &alias).unwrap();
        assert!(!policy.allows(&alias));
        let renamed = dir.path().join("renamed");
        fs::rename(&private, &renamed).unwrap();
        assert!(!policy.allows(&renamed.join("canary")));
    }

    #[test]
    fn persisted_rules_reopen_and_missing_or_tampered_state_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let private = dir.path().join("private");
        fs::create_dir(&private).unwrap();
        let path = dir.path().join("privacy.dpapi");
        let policy = AccessPolicy::open(path.clone()).unwrap();
        policy.replace_private_paths(vec![private.clone()]).unwrap();
        assert!(!AccessPolicy::open(path.clone()).unwrap().allows(&private));
        fs::write(&path, b"tampered").unwrap();
        assert!(AccessPolicy::open(path.clone()).is_err());
        fs::remove_file(&path).unwrap();
        assert!(AccessPolicy::open(path).is_err());
    }

    #[test]
    fn replacement_revokes_old_revision_and_thread_binding_restores_on_panic() {
        let dir = tempfile::tempdir().unwrap();
        let policy = Arc::new(AccessPolicy::default());
        let old = policy.revision();
        policy
            .replace_private_paths(vec![dir.path().to_path_buf()])
            .unwrap();
        assert!(policy
            .with_revision(old, || panic!("stale output"))
            .is_none());
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_policy(policy, || {
                assert!(check(dir.path()).is_err());
                panic!("worker failure");
            })
        }));
        assert!(check(dir.path()).is_ok());
    }

    #[test]
    fn scans_hashes_and_approvals_obey_the_same_live_policy() {
        use crate::{
            file_tasks::{FileAction, FileTaskPlan, FileTasks},
            largest::largest_files,
            walk::WalkLimits,
        };
        use std::sync::atomic::AtomicBool;
        let root = tempfile::tempdir().unwrap();
        let private = root.path().join("Private");
        fs::create_dir(&private).unwrap();
        let secret = private.join("canary.txt");
        fs::write(&secret, vec![1; 16000]).unwrap();
        let public = root.path().join("report.txt");
        fs::write(&public, b"public").unwrap();
        let destination = root.path().join("moved.txt");
        let policy = Arc::new(AccessPolicy::default());
        let plan = with_policy(policy.clone(), || {
            FileTaskPlan::prepare(FileAction::Move, Some(&public), &destination).unwrap()
        });
        policy.replace_private_paths(vec![private.clone()]).unwrap();
        with_policy(policy.clone(), || {
            let (files, stats) = largest_files(
                &[root.path().to_path_buf()],
                10,
                WalkLimits::whole_system_read(),
                &AtomicBool::new(false),
                &mut |_| {},
            );
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].path, public);
            assert!(stats.excluded_count > 0);
            assert!(crate::hashing::hash_file(&secret).is_err());
            let store = FileTasks::new(root.path().join("receipts"));
            assert!(
                store.execute_plan(&plan).is_err(),
                "old approval must be invalid after any policy change"
            );
            assert!(public.exists());
            assert!(!destination.exists());
            let receipt = store
                .execute(FileAction::Move, Some(&public), &destination)
                .unwrap();
            policy
                .replace_private_paths(vec![destination.clone()])
                .unwrap();
            assert!(
                store.undo(&receipt.id).is_err(),
                "undo must not read a newly private payload"
            );
            assert!(!public.exists());
            assert!(destination.exists());
        });
    }

    #[test]
    fn stale_preferences_cannot_overwrite_newer_rules() {
        let root = tempfile::tempdir().unwrap();
        let policy = AccessPolicy::default();
        policy
            .replace_if_revision(vec![root.path().to_path_buf()], 0)
            .unwrap();
        assert!(policy.replace_if_revision(vec![], 0).is_err());
        assert!(!policy.allows(root.path()));
    }

    #[test]
    fn revocation_stops_old_workers_even_when_nested_helpers_bind_again() {
        let root = tempfile::tempdir().unwrap();
        let private = root.path().join("private");
        fs::create_dir(&private).unwrap();
        let public = root.path().join("public.txt");
        fs::write(&public, "public").unwrap();
        let policy = Arc::new(AccessPolicy::default());
        with_policy(policy.clone(), || {
            assert!(check(&public).is_ok());
            policy.replace_private_paths(vec![private]).unwrap();
            assert!(
                check(&public).is_err(),
                "the old request has been revoked, including public reads"
            );
            with_policy(policy.clone(), || assert!(check(&public).is_err()));
        });
        with_policy(policy, || assert!(check(&public).is_ok()));
    }
}
