//! Windows-native encrypted persistence. No model-facing paths or mutable policy.
use crate::app_privacy::ApplicationPrivacyPolicy;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 512 * 1024;
const MARKER: &[u8] = b"custodian-app-privacy-v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema: u32,
    revision: u64,
    policy: ApplicationPrivacyPolicy,
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn missing_corrupt_oversized_and_unknown_saved_policy_never_defaults_open() {
        for fault in [
            "missing",
            "tamper",
            "oversize",
            "marker",
            "schema",
            "invalid-policy",
            "orphan",
        ] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("policy.dpapi");
            let (store, _, _) = Storage::open(path.clone()).unwrap();
            drop(store);
            match fault {
                "missing" => std::fs::remove_file(&path).unwrap(),
                "tamper" => std::fs::write(&path, b"corrupt-canary").unwrap(),
                "oversize" => std::fs::write(&path, vec![0; MAX_BYTES as usize + 1]).unwrap(),
                "marker" => std::fs::write(path.with_extension("required"), b"wrong").unwrap(),
                "orphan" => std::fs::remove_file(path.with_extension("required")).unwrap(),
                _ => {
                    let payload = if fault == "schema" {
                        serde_json::json!({"schema":2,"revision":0,"policy":{"private_executables":[],"private_processes":[]}})
                    } else {
                        serde_json::json!({"schema":1,"revision":0,"policy":{"private_executables":["relative.exe"],"private_processes":[]}})
                    };
                    std::fs::write(
                        &path,
                        custodian_winsec::dpapi::protect(&serde_json::to_vec(&payload).unwrap())
                            .unwrap(),
                    )
                    .unwrap();
                }
            }
            assert!(
                Storage::open(path).is_err(),
                "accepted corrupt/missing policy: {fault}"
            );
        }
    }
}

pub(crate) struct Storage {
    path: PathBuf,
    _lock: File,
}

fn invalid() -> io::Error {
    io::Error::other("Application privacy storage unavailable")
}

fn regular(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(invalid());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & (0x400 | 0x1000 | 0x0040_0000) != 0 {
            return Err(invalid());
        }
    }
    Ok(())
}

fn read_bounded(path: &Path) -> io::Result<Vec<u8>> {
    regular(path)?;
    let mut data = Vec::new();
    File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX_BYTES {
        return Err(invalid());
    }
    Ok(data)
}

impl Storage {
    pub(crate) fn open(path: PathBuf) -> io::Result<(Self, u64, ApplicationPrivacyPolicy)> {
        if !path.is_absolute() {
            return Err(invalid());
        }
        let parent = path.parent().ok_or_else(invalid)?;
        // The host selects its data directory. Refuse known redirected ancestors.
        for ancestor in parent.ancestors() {
            match regular(ancestor) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        fs::create_dir_all(parent)?;
        let lock_path = path.with_extension("lock");
        if lock_path.try_exists()? {
            regular(&lock_path)?;
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let lock = options.open(lock_path)?;
        let store = Self { path, _lock: lock };
        let marker = store.path.with_extension("required");
        let existing = store.path.try_exists()?;
        if !marker.try_exists()? {
            if existing {
                return Err(invalid());
            }
            // Establish missing-file detection before first usable policy. A crash
            // during initialization leaves startup denied, never empty defaults.
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker)?;
            file.write_all(MARKER)?;
            file.sync_all()?;
            store.save(0, &ApplicationPrivacyPolicy::default())?;
        }
        if read_bounded(&marker)? != MARKER {
            return Err(invalid());
        }
        let sealed = read_bounded(&store.path)?;
        let bytes = custodian_winsec::dpapi::unprotect(&sealed)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid());
        }
        let saved: Saved = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if saved.schema != 1 {
            return Err(invalid());
        }
        saved.policy.validate().map_err(|_| invalid())?;
        Ok((store, saved.revision, saved.policy))
    }

    pub(crate) fn save(&self, revision: u64, policy: &ApplicationPrivacyPolicy) -> io::Result<()> {
        if read_bounded(&self.path.with_extension("required"))? != MARKER {
            return Err(invalid());
        }
        if self.path.try_exists()? {
            regular(&self.path)?;
        }
        let bytes = serde_json::to_vec(&Saved {
            schema: 1,
            revision,
            policy: policy.clone(),
        })
        .map_err(|_| invalid())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid());
        }
        let sealed = custodian_winsec::dpapi::protect(&bytes)?;
        if sealed.len() as u64 > MAX_BYTES {
            return Err(invalid());
        }
        let temporary = self
            .path
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| {
            file.write_all(&sealed)?;
            file.sync_all()?;
            drop(file);
            custodian_winsec::settings_commit::replace(&temporary, &self.path)
        })();
        if result.is_err() {
            // Only this invocation's create-new temporary file is eligible.
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
