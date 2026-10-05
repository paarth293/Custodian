//! The ledger's anchor: its height and head hash, kept outside the ledger.
//!
//! The chain shows any edit to a record, but not records cut from the end.
//! Saving the anchor after every append and checking it with
//! [`crate::chain::Ledger::verify_anchor`] closes that gap. The anchor is
//! sealed with DPAPI, so it cannot be edited to match a shortened ledger
//! without the user's keys. (The design's TPM signature is not used yet.)

use crate::chain::Hash;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"CLA1";
const LEN: usize = 4 + 8 + 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub height: u64,
    pub head: Hash,
}

pub struct AnchorManager {
    path: PathBuf,
}

impl AnchorManager {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Seals and atomically replaces the saved anchor.
    pub fn save(&self, anchor: &Anchor) -> io::Result<()> {
        let mut plain = Vec::with_capacity(LEN);
        plain.extend_from_slice(MAGIC);
        plain.extend_from_slice(&anchor.height.to_le_bytes());
        plain.extend_from_slice(&anchor.head);
        let sealed = custodian_winsec::dpapi::protect(&plain)?;
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("tmp");
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&sealed)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, &self.path)
    }

    /// The saved anchor, `None` before the first save. A modified anchor file
    /// is an error, never silently ignored.
    pub fn load(&self) -> io::Result<Option<Anchor>> {
        let sealed = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        let corrupt = || io::Error::new(io::ErrorKind::InvalidData, "ledger anchor was modified");
        let plain = custodian_winsec::dpapi::unprotect(&sealed).map_err(|_| corrupt())?;
        if plain.len() != LEN || &plain[..4] != MAGIC {
            return Err(corrupt());
        }
        let mut height = [0u8; 8];
        height.copy_from_slice(&plain[4..12]);
        let mut head = [0u8; 32];
        head.copy_from_slice(&plain[12..]);
        Ok(Some(Anchor {
            height: u64::from_le_bytes(height),
            head,
        }))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_sealed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor.dpapi");
        let manager = AnchorManager::new(&path);
        assert_eq!(manager.load().unwrap(), None);

        let anchor = Anchor {
            height: 42,
            head: [9u8; 32],
        };
        manager.save(&anchor).unwrap();
        assert_eq!(manager.load().unwrap(), Some(anchor));

        let mut sealed = fs::read(&path).unwrap();
        assert!(!sealed.windows(32).any(|w| w == [9u8; 32]));
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        fs::write(&path, sealed).unwrap();
        assert!(manager.load().is_err());
    }
}
