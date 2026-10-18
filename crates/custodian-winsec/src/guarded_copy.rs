//! Windows copy guards keep the source and ancestor objects open through verification.
//! This is not protection against privileged code or arbitrary writable mappings.
use crate::fsops::FileIdentity;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};

pub struct CopyGuard {
    _directories: Vec<File>,
    source: File,
    destination: PathBuf,
    output: Option<File>,
}

#[cfg(windows)]
impl CopyGuard {
    pub fn open(source: &Path, destination: &Path) -> io::Result<Self> {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        };
        let invalid = || io::Error::other("copy requires ordinary absolute local paths");
        for path in [source, destination] {
            if !path.is_absolute()
                || !matches!(path.components().next(), Some(std::path::Component::Prefix(p))
                    if matches!(p.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
                || path.components().any(|c| match c {
                    std::path::Component::ParentDir | std::path::Component::CurDir => true,
                    std::path::Component::Normal(n) => {
                        let n = n.to_string_lossy();
                        n.contains([':', '\0']) || n.ends_with(['.', ' '])
                    }
                    _ => false,
                })
            {
                return Err(invalid());
            }
        }
        let mut directories = Vec::new();
        // Open root-to-leaf, rejecting reparse/cloud objects on the opened
        // handles. No delete/write sharing: other ordinary handles cannot
        // rename these ancestors or turn them into write-opened junctions.
        for parent in [
            source.parent().ok_or_else(invalid)?,
            destination.parent().ok_or_else(invalid)?,
        ] {
            for ancestor in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
                let file = OpenOptions::new()
                    .access_mode(0x80)
                    .share_mode(1)
                    .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                    .open(ancestor)?;
                let meta = file.metadata()?;
                if !meta.is_dir()
                    || meta.file_attributes() & (0x400 | 0x1000 | 0x40000 | 0x400000) != 0
                {
                    return Err(invalid());
                }
                directories.push(file);
            }
        }
        let input = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(source)?;
        let meta = input.metadata()?;
        if !meta.is_file()
            || meta.file_attributes() & (0x400 | 0x1000 | 0x40000 | 0x400000) != 0
            || crate::fsops::file_link_count(source)? != 1
        {
            return Err(invalid());
        }
        Ok(Self {
            _directories: directories,
            source: input,
            destination: destination.into(),
            output: None,
        })
    }
}

#[cfg(not(windows))]
impl CopyGuard {
    pub fn open(_source: &Path, _destination: &Path) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "guarded copies currently require Windows",
        ))
    }
}

impl CopyGuard {
    /// Caller persists its prepared receipt before this first filesystem write.
    pub fn copy_verified(
        &mut self,
        expected_bytes: u64,
        verify: impl Fn(&mut File) -> io::Result<bool>,
    ) -> io::Result<FileIdentity> {
        self.copy_verified_with_progress(expected_bytes, verify, &mut |_| Ok(()))
    }

    /// Reports durable-executor copy progress without granting permission to
    /// overwrite or dropping the source/ancestor handles. On any I/O or observer
    /// failure, leave the partial file for receipt-based recovery.
    pub fn copy_verified_with_progress(
        &mut self,
        expected_bytes: u64,
        verify: impl Fn(&mut File) -> io::Result<bool>,
        progress: &mut dyn FnMut(u64) -> io::Result<()>,
    ) -> io::Result<FileIdentity> {
        self.source.rewind()?;
        if self.source.metadata()?.len() != expected_bytes || !verify(&mut self.source)? {
            return Err(io::Error::other("source changed; no copy was started"));
        }
        self.source.rewind()?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1);
        }
        self.output = Some(options.open(&self.destination)?);
        let output = self.output.as_mut().unwrap();
        let mut copied = 0_u64;
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            let count = self.source.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
            copied += count as u64;
            progress(copied)?;
        }
        output.sync_all()?;
        output.rewind()?;
        self.source.rewind()?;
        if copied != expected_bytes || !verify(output)? || !verify(&mut self.source)? {
            return Err(io::Error::other(
                "copy verification failed; retain the recovery receipt",
            ));
        }
        crate::fsops::file_identity(&self.destination)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn copy_guard_blocks_source_and_parent_replacement_and_preserves_conflicts() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("work");
        std::fs::create_dir(&folder).unwrap();
        let source = folder.join("source.txt");
        let target = folder.join("copy.txt");
        std::fs::write(&source, b"payload").unwrap();
        let mut guard = CopyGuard::open(&source, &target).unwrap();
        assert!(std::fs::write(&source, b"replacement").is_err());
        assert!(std::fs::rename(&source, folder.join("swapped.txt")).is_err());
        assert!(std::fs::rename(&folder, root.path().join("swapped")).is_err());
        guard
            .copy_verified(7, |file| {
                use std::io::Read;
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                Ok(bytes == b"payload")
            })
            .unwrap();
        assert!(std::fs::write(&target, b"overwrite").is_err());
        drop(guard);
        assert_eq!(std::fs::read(&source).unwrap(), b"payload");
        assert_eq!(std::fs::read(&target).unwrap(), b"payload");
        let mut guard = CopyGuard::open(&source, &target).unwrap();
        assert!(guard.copy_verified(7, |_| Ok(true)).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"payload");
    }
}
