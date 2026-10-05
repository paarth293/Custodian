use custodian_winsec::fsops::move_no_replace;
use custodian_winsec::safe_place::SafePlace;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

/// Moves files into the vault and restores them on undo (build plan step 60).
///
/// A move within a volume is a rename, so no bytes are copied; across volumes
/// the file is copied, verified and only then removed. Neither direction ever
/// replaces an existing file. Each move leaves a sidecar, sealed with DPAPI,
/// holding the content hash that the undo checks before putting the file back.
pub struct QuarantineOp {
    vault: SafePlace,
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.to_string())
}

fn hash_file(path: &Path) -> io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(File::open(path)?)?;
    Ok(hasher.finalize().to_hex().to_string())
}

/// Moves `from` to `to` without replacing anything, checking a cross-volume
/// copy against `hash` before the original is removed.
fn move_verified(from: &Path, to: &Path, hash: &str) -> io::Result<()> {
    move_no_replace(from, to, |copy| Ok(hash_file(copy)? == hash))
}

impl QuarantineOp {
    pub fn new(vault_path: PathBuf) -> io::Result<Self> {
        Ok(Self {
            vault: SafePlace::new(vault_path)?,
        })
    }

    pub fn execute(&self, source: &Path, id: &str) -> io::Result<()> {
        Self::check_id(id)?;
        if fs::symlink_metadata(source)?.file_type().is_symlink() {
            return Err(invalid("refusing to quarantine a link"));
        }
        let payload = self.payload_path(id);
        if payload.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "transaction id already used",
            ));
        }
        let sidecar = Self::sidecar_name(id);
        let hash = hash_file(source)?;
        // The sidecar is written first: a crash before the move leaves the
        // file where it was.
        self.vault.write_encrypted(&sidecar, hash.as_bytes())?;
        if let Err(err) = move_verified(source, &payload, &hash) {
            let _ = fs::remove_file(self.vault.root().join(&sidecar));
            return Err(err);
        }
        // A rename moves whatever the file holds at that instant. If it was
        // written to after hashing, record what was actually quarantined so
        // the undo can still restore it.
        let moved = hash_file(&payload)?;
        if moved != hash {
            self.vault.write_encrypted(&sidecar, moved.as_bytes())?;
        }
        Ok(())
    }

    pub fn inverse(&self, target: &Path, id: &str) -> io::Result<()> {
        Self::check_id(id)?;
        let sidecar = Self::sidecar_name(id);
        let hash = String::from_utf8(self.vault.read_decrypted(&sidecar)?)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "corrupt sidecar"))?;
        let payload = self.payload_path(id);
        if hash_file(&payload)? != hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the quarantined file no longer matches its hash",
            ));
        }
        // Never replaces a file that appeared at the original location.
        move_verified(&payload, target, &hash)?;
        fs::remove_file(self.vault.root().join(sidecar))
    }

    /// Transaction ids become file names, so only plain names are allowed.
    fn check_id(id: &str) -> io::Result<()> {
        let ok = !id.is_empty()
            && id.len() <= 64
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if ok {
            Ok(())
        } else {
            Err(invalid("invalid transaction id"))
        }
    }

    fn payload_path(&self, id: &str) -> PathBuf {
        self.vault.root().join(format!("{id}.bin"))
    }

    fn sidecar_name(id: &str) -> String {
        format!("{id}.sidecar")
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn setup(content: &[u8]) -> (tempfile::TempDir, PathBuf, QuarantineOp) {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("secret.txt");
        fs::write(&source, content).unwrap();
        let op = QuarantineOp::new(dir.path().join("vault")).unwrap();
        (dir, source, op)
    }

    #[test]
    fn roundtrip() {
        let (_dir, source, op) = setup(b"confidential data");
        op.execute(&source, "tx123").unwrap();
        assert!(!source.exists());
        assert_eq!(
            fs::read(op.payload_path("tx123")).unwrap(),
            b"confidential data"
        );

        op.inverse(&source, "tx123").unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"confidential data");
        assert!(!op.payload_path("tx123").exists());
    }

    #[test]
    fn undo_never_overwrites_a_new_file() {
        let (_dir, source, op) = setup(b"old");
        op.execute(&source, "tx1").unwrap();
        fs::write(&source, b"new").unwrap();
        assert!(op.inverse(&source, "tx1").is_err());
        assert_eq!(fs::read(&source).unwrap(), b"new");
    }

    #[test]
    fn a_locked_file_is_left_alone_with_no_trace() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        let (_dir, source, op) = setup(b"in use");
        // Another program has the file open: others may read it, not move it.
        let _held = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&source)
            .unwrap();
        assert!(op.execute(&source, "tx1").is_err());
        assert_eq!(fs::read(&source).unwrap(), b"in use");
        assert!(!op.payload_path("tx1").exists());
        assert!(!op
            .vault
            .root()
            .join(QuarantineOp::sidecar_name("tx1"))
            .exists());
    }

    #[test]
    fn undo_refuses_a_changed_payload() {
        let (_dir, source, op) = setup(b"original");
        op.execute(&source, "tx1").unwrap();
        fs::write(op.payload_path("tx1"), b"swapped").unwrap();
        assert!(op.inverse(&source, "tx1").is_err());
        assert!(!source.exists());
    }

    #[test]
    fn rejects_path_like_ids_and_reused_ids() {
        let (_dir, source, op) = setup(b"x");
        for bad in ["", "../escape", "a/b", "a\\b", "x.y"] {
            assert!(op.execute(&source, bad).is_err(), "{bad}");
        }
        assert!(source.exists());
        op.execute(&source, "tx1").unwrap();
        fs::write(&source, b"y").unwrap();
        assert!(op.execute(&source, "tx1").is_err());
        assert_eq!(fs::read(&source).unwrap(), b"y");
    }
}
