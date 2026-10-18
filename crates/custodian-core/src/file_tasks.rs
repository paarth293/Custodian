//! Sealed write-ahead receipts and verified undo for approved desktop file tasks.
//! This module does not grant authority: callers must obtain an exact approval
//! or enforce a current bounded standing permission before execution.

use crate::hashing::hash_file;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileAction {
    Move,
    Copy,
    Rename,
    CreateFolder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptState {
    Prepared,
    Completed,
    Undone,
    Failed,
    RecoveryRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReceipt {
    pub id: String,
    pub action: FileAction,
    pub source: Option<PathBuf>,
    pub destination: PathBuf,
    pub hash: Option<String>,
    pub bytes: u64,
    pub created_order: u64,
    pub state: ReceiptState,
    #[serde(default)]
    pub destination_identity: Option<custodian_winsec::fsops::FileIdentity>,
}

pub struct FileTasks {
    root: PathBuf,
}

#[derive(Clone, Copy)]
pub struct CopyLimits {
    pub max_bytes: u64,
    pub min_free_bytes: u64,
    pub max_space_percent: u8,
}

/// Exact objects and content shown for approval. Preparing this never writes.
#[derive(Debug, Clone)]
pub struct FileTaskPlan {
    policy_revision: u64,
    pub action: FileAction,
    pub source: Option<PathBuf>,
    pub destination: PathBuf,
    source_identity: Option<custodian_winsec::fsops::FileIdentity>,
    parent_identity: custodian_winsec::fsops::FileIdentity,
    hash: Option<String>,
    bytes: u64,
}

impl FileTaskPlan {
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    pub fn prepare(
        action: FileAction,
        source: Option<&Path>,
        destination: &Path,
    ) -> io::Result<Self> {
        crate::privacy::check(destination)?;
        if let Some(source) = source {
            crate::privacy::check(source)?;
        }
        let _copy_guard = if action == FileAction::Copy {
            Some(custodian_winsec::guarded_copy::CopyGuard::open(
                source.ok_or_else(|| invalid("a source file is required"))?,
                destination,
            )?)
        } else {
            None
        };
        let destination = checked_destination(destination)?;
        if let Some(source) = source {
            check_regular(source)?;
        }
        let source = source.map(fs::canonicalize).transpose()?;
        let (source_identity, hash, bytes) = match (action, source.as_deref()) {
            (FileAction::CreateFolder, None) => (None, None, 0),
            (FileAction::CreateFolder, Some(_)) => {
                return Err(invalid("folder creation does not take a source"));
            }
            (_, Some(source)) => {
                check_regular(source)?;
                if !custodian_winsec::fsops::is_local_volume(source) {
                    return Err(invalid("offline file tasks require a local source volume"));
                }
                (
                    Some(custodian_winsec::fsops::file_identity(source)?),
                    Some(hash_file(source)?),
                    fs::metadata(source)?.len(),
                )
            }
            (_, None) => return Err(invalid("a source file is required")),
        };
        let parent_identity =
            custodian_winsec::fsops::file_identity(destination.parent().unwrap())?;
        Ok(Self {
            policy_revision: crate::privacy::current().revision(),
            action,
            source,
            destination,
            source_identity,
            parent_identity,
            hash,
            bytes,
        })
    }

    pub(crate) fn validate(&self) -> io::Result<()> {
        if crate::privacy::current().revision() != self.policy_revision {
            return Err(invalid("privacy rules changed since approval; ask again"));
        }
        let destination = checked_destination(&self.destination)?;
        if destination != self.destination
            || custodian_winsec::fsops::file_identity(destination.parent().unwrap())?
                != self.parent_identity
        {
            return Err(invalid(
                "the destination folder changed since approval; ask again",
            ));
        }
        if let Some(source) = self.source.as_deref() {
            check_regular(source)?;
            if custodian_winsec::fsops::file_identity(source)? != self.source_identity.unwrap()
                || fs::metadata(source)?.len() != self.bytes
                || hash_file(source)? != self.hash.as_deref().unwrap()
            {
                return Err(invalid("the source file changed since approval; ask again"));
            }
        }
        // Approval does not waive capacity protection. A same-volume rename
        // does not duplicate content; copies and cross-volume moves do.
        if self.action == FileAction::Copy
            || self
                .source_identity
                .is_some_and(|source| source.volume != self.parent_identity.volume)
        {
            check_capacity(self.bytes, self.destination.parent().unwrap())?;
        }
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn check_capacity(bytes: u64, destination_parent: &Path) -> io::Result<()> {
    let free = custodian_winsec::fsops::available_bytes(destination_parent)?;
    #[cfg(all(test, windows))]
    let free = crate::crash_probe::free_bytes(free);
    if free
        .checked_sub(bytes)
        .is_none_or(|left| left < crate::routine::FREE_RESERVE)
    {
        return Err(invalid(
            "The file operation would exceed destination capacity or its 1 GiB free-space reserve",
        ));
    }
    Ok(())
}
fn check_regular(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if matches!(path.components().next(), Some(std::path::Component::Prefix(prefix)) if !matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
    {
        return Err(invalid(
            "network and device paths are not supported for offline file tasks",
        ));
    }
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid("only ordinary local files are supported"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & (0x400 | 0x1000 | 0x0040_0000) != 0 {
            return Err(invalid(
                "cloud placeholders and reparse points are not supported",
            ));
        }
    }
    Ok(())
}

fn checked_destination(path: &Path) -> io::Result<PathBuf> {
    #[cfg(windows)]
    if matches!(path.components().next(), Some(std::path::Component::Prefix(prefix)) if !matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
    {
        return Err(invalid(
            "network and device paths are not supported for offline file tasks",
        ));
    }
    if path
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(invalid("parent traversal is not allowed"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| invalid("a parent folder is required"))?;
    let parent = fs::canonicalize(parent)?;
    if !custodian_winsec::fsops::is_local_volume(&parent) {
        return Err(invalid("offline file tasks require a local volume"));
    }
    let name = path
        .file_name()
        .ok_or_else(|| invalid("a file or folder name is required"))?;
    let result = parent.join(name);
    if fs::symlink_metadata(&result).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "the destination already exists",
        ));
    }
    Ok(result)
}

impl FileTasks {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn save(&self, receipt: &FileReceipt) -> io::Result<()> {
        if uuid::Uuid::parse_str(&receipt.id).is_err() {
            return Err(invalid("invalid receipt identity"));
        }
        fs::create_dir_all(&self.root)?;
        let plain = serde_json::to_vec(receipt).map_err(io::Error::other)?;
        let sealed = custodian_winsec::dpapi::protect(&plain)?;
        let path = self.root.join(format!("{}.dpapi", receipt.id));
        let temporary = path.with_extension("tmp");
        let mut file = File::create(&temporary)?;
        file.write_all(&sealed)?;
        file.sync_all()?;
        drop(file);
        fs::rename(temporary, path)
    }

    pub fn receipts(&self) -> io::Result<Vec<FileReceipt>> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err),
        };
        let mut receipts = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path
                .extension()
                .is_none_or(|extension| extension != "dpapi")
            {
                continue;
            }
            let plain = custodian_winsec::dpapi::unprotect(&fs::read(&path)?)?;
            let receipt: FileReceipt = serde_json::from_slice(&plain).map_err(io::Error::other)?;
            if path
                .file_stem()
                .is_none_or(|name| name != receipt.id.as_str())
                || uuid::Uuid::parse_str(&receipt.id).is_err()
            {
                return Err(invalid("receipt identity does not match its journal file"));
            }
            receipts.push(receipt);
        }
        receipts.sort_by_key(|receipt| std::cmp::Reverse(receipt.created_order));
        Ok(receipts)
    }

    /// The prepared inverse is durable before the first filesystem mutation.
    pub fn execute(
        &self,
        action: FileAction,
        source: Option<&Path>,
        destination: &Path,
    ) -> io::Result<FileReceipt> {
        self.execute_plan(&FileTaskPlan::prepare(action, source, destination)?)
    }

    /// Recheck the approved snapshot rather than resolving its names a second time.
    pub fn execute_plan(&self, plan: &FileTaskPlan) -> io::Result<FileReceipt> {
        crate::privacy::with_mutation(|| self.execute_checked(plan, None, None))
    }

    pub fn execute_bounded_copy(
        &self,
        plan: &FileTaskPlan,
        limits: CopyLimits,
        cancel: &AtomicBool,
    ) -> io::Result<FileReceipt> {
        if plan.action != FileAction::Copy || !(1..=100).contains(&limits.max_space_percent) {
            return Err(invalid("invalid bounded-copy request"));
        }
        crate::privacy::with_mutation(|| self.execute_checked(plan, Some((limits, cancel)), None))
    }

    /// A workflow journals this ID before calling the executor, closing the
    /// crash gap between file completion and its parent step's completion.
    pub fn execute_workflow_copy(
        &self,
        plan: &FileTaskPlan,
        limits: CopyLimits,
        cancel: &AtomicBool,
        receipt_id: &str,
    ) -> io::Result<FileReceipt> {
        if plan.action != FileAction::Copy
            || !(1..=100).contains(&limits.max_space_percent)
            || !uuid::Uuid::parse_str(receipt_id).is_ok_and(|value| value.to_string() == receipt_id)
        {
            return Err(invalid("invalid workflow copy"));
        }
        crate::privacy::with_mutation(|| {
            self.execute_checked(plan, Some((limits, cancel)), Some(receipt_id))
        })
    }

    /// Inspect without replaying a mutation. Used after restart and before
    /// reporting a whole workflow verified.
    pub fn verify_copy(&self, receipt: &FileReceipt) -> io::Result<()> {
        if receipt.action != FileAction::Copy || receipt.state != ReceiptState::Completed {
            return Err(invalid("copy is not completed"));
        }
        let source = receipt
            .source
            .as_deref()
            .ok_or_else(|| invalid("missing source"))?;
        crate::privacy::check(source)?;
        crate::privacy::check(&receipt.destination)?;
        let _guard = custodian_winsec::guarded_copy::CopyGuard::open(source, &receipt.destination)?;
        let _output_guard =
            custodian_winsec::guarded_copy::CopyGuard::open(&receipt.destination, source)?;
        if Some(custodian_winsec::fsops::file_identity(
            &receipt.destination,
        )?) != receipt.destination_identity
            || fs::metadata(source)?.len() != receipt.bytes
            || fs::metadata(&receipt.destination)?.len() != receipt.bytes
            || Some(hash_file(source)?) != receipt.hash
            || Some(hash_file(&receipt.destination)?) != receipt.hash
        {
            return Err(invalid("source or copy changed; verification failed"));
        }
        Ok(())
    }

    fn execute_checked(
        &self,
        plan: &FileTaskPlan,
        bounded: Option<(CopyLimits, &AtomicBool)>,
        receipt_id: Option<&str>,
    ) -> io::Result<FileReceipt> {
        crate::privacy::check(&plan.destination)?;
        if let Some(source) = &plan.source {
            crate::privacy::check(source)?;
        }
        let _writer = crate::mutation_lock::MUTATIONS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // A corrupt or inaccessible prior journal is a recovery problem,
        // never permission to append new mutations beside unreadable history.
        let receipts = self.receipts()?;
        if let Some(id) = receipt_id {
            if receipts.iter().any(|receipt| receipt.id == id) {
                return Err(invalid(
                    "workflow receipt already exists; inspect instead of replaying",
                ));
            }
        }
        let mut copy_guard = if plan.action == FileAction::Copy {
            Some(custodian_winsec::guarded_copy::CopyGuard::open(
                plan.source
                    .as_deref()
                    .ok_or_else(|| invalid("missing source"))?,
                &plan.destination,
            )?)
        } else {
            None
        };
        plan.validate()?;
        if let Some((limits, cancel)) = bounded {
            let free =
                custodian_winsec::fsops::available_bytes(plan.destination.parent().unwrap())?;
            if plan.bytes > limits.max_bytes
                || u128::from(plan.bytes) * 100
                    > u128::from(free) * u128::from(limits.max_space_percent)
                || free.saturating_sub(plan.bytes) < limits.min_free_bytes
            {
                return Err(invalid(
                    "The backup exceeds its size or destination free-space budget",
                ));
            }
            if cancel.load(Ordering::Relaxed) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Backup stopped before copying",
                ));
            }
        }
        let action = plan.action;
        let mut receipt = FileReceipt {
            id: receipt_id
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            action,
            source: plan.source.clone(),
            destination: plan.destination.clone(),
            hash: plan.hash.clone(),
            bytes: plan.bytes,
            created_order: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .min(u64::MAX as u128) as u64,
            state: ReceiptState::Prepared,
            destination_identity: None,
        };
        self.save(&receipt)?;
        #[cfg(all(test, windows))]
        crate::crash_probe::checkpoint("before-mutation")?;
        let result = if bounded.is_some_and(|(_, cancel)| cancel.load(Ordering::Relaxed)) {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Backup stopped before copying",
            ))
        } else {
            match action {
                FileAction::CreateFolder => fs::create_dir(&receipt.destination),
                FileAction::Move | FileAction::Rename => {
                    let expected = receipt.hash.as_deref().unwrap();
                    custodian_winsec::fsops::move_no_replace(
                        receipt.source.as_deref().unwrap(),
                        &receipt.destination,
                        |path| Ok(hash_file(path)? == expected),
                    )
                }
                FileAction::Copy => {
                    let expected = receipt.hash.as_deref().unwrap();
                    copy_guard
                        .as_mut()
                        .unwrap()
                        .copy_verified_with_progress(
                            receipt.bytes,
                            |file| Ok(crate::hashing::hash_reader(file)? == expected),
                            &mut |_copied| {
                                #[cfg(all(test, windows))]
                                crate::crash_probe::checkpoint("mid-copy")?;
                                Ok(())
                            },
                        )
                        .map(|_| ())
                }
            }
        };
        if let Err(err) = result {
            receipt.state = if receipt.destination.exists() {
                ReceiptState::RecoveryRequired
            } else {
                ReceiptState::Failed
            };
            self.save(&receipt)?;
            return Err(err);
        }
        receipt.destination_identity = Some(custodian_winsec::fsops::file_identity(
            &receipt.destination,
        )?);
        receipt.created_order = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .min(u64::MAX as u128) as u64;
        receipt.state = ReceiptState::Completed;
        self.save(&receipt).map_err(|err| io::Error::other(format!("The file operation ran, but its completion record could not be saved: {err}. Its prepared recovery record remains.")))?;
        #[cfg(all(test, windows))]
        crate::crash_probe::checkpoint("child-persisted")?;
        Ok(receipt)
    }

    pub fn undo_last(&self) -> io::Result<FileReceipt> {
        let receipt = self
            .receipts()?
            .into_iter()
            .find(|receipt| receipt.state == ReceiptState::Completed)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "no file task is available to undo")
            })?;
        self.undo(&receipt.id)
    }

    pub fn undo(&self, id: &str) -> io::Result<FileReceipt> {
        crate::privacy::with_mutation(|| self.undo_checked(id))
    }

    fn undo_checked(&self, id: &str) -> io::Result<FileReceipt> {
        let _writer = crate::mutation_lock::MUTATIONS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut receipt = self
            .receipts()?
            .into_iter()
            .find(|receipt| receipt.id == id && receipt.state == ReceiptState::Completed)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "no file task is available to undo")
            })?;
        let identity = receipt.destination_identity.ok_or_else(|| {
            invalid("the receipt has no verified file identity; manual recovery is required")
        })?;
        crate::privacy::check(&receipt.destination)?;
        if let Some(source) = &receipt.source {
            crate::privacy::check(source)?;
        }
        let _copy_guard = if receipt.action == FileAction::Copy {
            Some(custodian_winsec::guarded_copy::CopyGuard::open(
                receipt
                    .source
                    .as_deref()
                    .ok_or_else(|| invalid("missing source"))?,
                &receipt.destination,
            )?)
        } else {
            None
        };
        if custodian_winsec::fsops::file_identity(&receipt.destination)? != identity {
            return Err(invalid("the destination was replaced; it was left alone"));
        }
        match receipt.action {
            FileAction::CreateFolder => {
                let meta = fs::symlink_metadata(&receipt.destination)?;
                if !meta.is_dir() || meta.file_type().is_symlink() {
                    return Err(invalid("the created folder changed; it was left alone"));
                }
                // Windows refuses a nonempty directory. The same opened object's
                // identity is checked, so a replacement directory cannot be removed.
                custodian_winsec::fsops::remove_verified(&receipt.destination, identity, |_| {
                    Ok(true)
                })?;
            }
            FileAction::Move | FileAction::Rename => {
                check_regular(&receipt.destination)?;
                let source = receipt
                    .source
                    .as_deref()
                    .ok_or_else(|| invalid("missing original location"))?;
                if fs::symlink_metadata(source).is_ok() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "the original location is occupied; nothing was overwritten",
                    ));
                }
                let expected = receipt
                    .hash
                    .as_deref()
                    .ok_or_else(|| invalid("missing content hash"))?;
                if hash_file(&receipt.destination)? != expected {
                    return Err(invalid("the moved file changed; it was left alone"));
                }
                let parent = source
                    .parent()
                    .ok_or_else(|| invalid("missing original parent"))?;
                if custodian_winsec::fsops::file_identity(parent)?.volume != identity.volume {
                    check_capacity(receipt.bytes, parent)?;
                }
                custodian_winsec::fsops::move_no_replace(&receipt.destination, source, |path| {
                    Ok(hash_file(path)? == expected)
                })?;
            }
            FileAction::Copy => {
                let source = receipt
                    .source
                    .as_deref()
                    .ok_or_else(|| invalid("missing original location"))?;
                let expected = receipt
                    .hash
                    .as_deref()
                    .ok_or_else(|| invalid("missing content hash"))?;
                check_regular(source)?;
                check_regular(&receipt.destination)?;
                let mut options = OpenOptions::new();
                options.read(true);
                #[cfg(windows)]
                {
                    use std::os::windows::fs::OpenOptionsExt;
                    const FILE_SHARE_READ: u32 = 1;
                    options.share_mode(FILE_SHARE_READ);
                }
                let mut kept = options.open(source)?;
                if crate::hashing::hash_reader(&mut kept)? != expected {
                    return Err(invalid(
                        "the original or copied file changed; both were left alone",
                    ));
                }
                custodian_winsec::fsops::remove_verified(&receipt.destination, identity, |file| {
                    Ok(crate::hashing::hash_reader(file)? == expected)
                })?;
                drop(kept);
            }
        }
        receipt.state = ReceiptState::Undone;
        self.save(&receipt)?;
        Ok(receipt)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn unavailable_destination_and_corrupt_history_refuse_new_mutations() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let parent = dir.path().join("destination");
        fs::write(&source, b"unchanged original").unwrap();
        fs::create_dir(&parent).unwrap();
        let plan = FileTaskPlan::prepare(FileAction::Copy, Some(&source), &parent.join("copy.txt"))
            .unwrap();
        fs::remove_dir(&parent).unwrap();
        let tasks = FileTasks::new(dir.path().join("journal"));
        assert!(tasks.execute_plan(&plan).is_err());
        assert!(tasks.receipts().unwrap().is_empty());
        fs::create_dir(&parent).unwrap();
        let plan = FileTaskPlan::prepare(FileAction::Copy, Some(&source), &parent.join("copy.txt"))
            .unwrap();
        fs::create_dir(dir.path().join("journal")).unwrap();
        fs::write(
            dir.path()
                .join("journal")
                .join(format!("{}.dpapi", uuid::Uuid::new_v4())),
            b"corrupt sealed record",
        )
        .unwrap();
        assert!(tasks.execute_plan(&plan).is_err());
        assert!(!parent.join("copy.txt").exists());
        assert_eq!(fs::read(source).unwrap(), b"unchanged original");
    }
    #[test]
    fn approval_rejects_changed_content_and_same_content_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("destination.txt");
        fs::write(&source, b"approved bytes").unwrap();
        let tasks = FileTasks::new(dir.path().join("receipts"));
        let plan = FileTaskPlan::prepare(FileAction::Move, Some(&source), &destination).unwrap();
        fs::write(&source, b"modified bytes").unwrap();
        assert!(tasks.execute_plan(&plan).is_err());
        assert!(!destination.exists());
        fs::write(&source, b"approved bytes").unwrap();
        // Retain the old object so its identity cannot be reused by the filesystem.
        fs::rename(&source, dir.path().join("original.txt")).unwrap();
        fs::write(&source, b"approved bytes").unwrap();
        assert!(tasks.execute_plan(&plan).is_err());
        assert!(!destination.exists());
        assert!(tasks.receipts().unwrap().is_empty());
    }

    #[test]
    fn approval_rejects_replaced_destination_parent_and_occupied_destination() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let parent = dir.path().join("target");
        fs::write(&source, b"approved bytes").unwrap();
        fs::create_dir(&parent).unwrap();
        let destination = parent.join("result.txt");
        let tasks = FileTasks::new(dir.path().join("receipts"));
        let plan = FileTaskPlan::prepare(FileAction::Copy, Some(&source), &destination).unwrap();
        fs::write(&destination, b"someone else's work").unwrap();
        assert!(tasks.execute_plan(&plan).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"someone else's work");
        fs::rename(&parent, dir.path().join("old-target")).unwrap();
        fs::create_dir(&parent).unwrap();
        assert!(tasks.execute_plan(&plan).is_err());
        assert!(!destination.exists());
        assert!(source.exists());
    }

    #[test]
    fn approved_move_can_be_verified_reopened_and_undone() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("destination.txt");
        let journal = dir.path().join("receipts");
        fs::write(&source, b"recoverable payload").unwrap();
        let plan = FileTaskPlan::prepare(FileAction::Move, Some(&source), &destination).unwrap();
        assert!(source.exists());
        assert!(!destination.exists());
        assert!(!journal.exists(), "preparation must be read-only");
        let receipt = FileTasks::new(journal.clone()).execute_plan(&plan).unwrap();
        assert_eq!(receipt.state, ReceiptState::Completed);
        assert!(!source.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"recoverable payload");
        // Replay of the same plan must not create another mutation.
        assert!(FileTasks::new(journal.clone()).execute_plan(&plan).is_err());
        FileTasks::new(journal.clone()).undo(&receipt.id).unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"recoverable payload");
        assert!(!destination.exists());
        assert_eq!(
            FileTasks::new(journal).receipts().unwrap()[0].state,
            ReceiptState::Undone
        );
    }

    #[test]
    fn undo_refuses_a_replacement_with_the_same_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("copy");
        fs::write(&source, b"same bytes").unwrap();
        let tasks = FileTasks::new(dir.path().join("journal"));
        tasks
            .execute(FileAction::Copy, Some(&source), &destination)
            .unwrap();
        fs::rename(&destination, dir.path().join("old-copy")).unwrap();
        fs::write(&destination, b"same bytes").unwrap();
        assert!(tasks.undo_last().is_err());
        assert_eq!(fs::read(destination).unwrap(), b"same bytes");
    }

    #[test]
    fn prepared_folder_receipt_cannot_remove_a_folder_created_by_someone_else() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("folder");
        let tasks = FileTasks::new(dir.path().join("journal"));
        let receipt = FileReceipt {
            id: uuid::Uuid::new_v4().to_string(),
            action: FileAction::CreateFolder,
            source: None,
            destination: destination.clone(),
            hash: None,
            bytes: 0,
            created_order: 1,
            state: ReceiptState::Prepared,
            destination_identity: None,
        };
        tasks.save(&receipt).unwrap();
        fs::create_dir(&destination).unwrap();
        assert!(tasks.undo_last().is_err());
        assert!(destination.is_dir());
    }
    #[test]
    fn each_file_task_can_be_undone_after_reopening_its_journal() {
        for action in [
            FileAction::Move,
            FileAction::Copy,
            FileAction::Rename,
            FileAction::CreateFolder,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("original.txt");
            let destination = dir.path().join("output");
            fs::write(&source, b"verified content").unwrap();
            let journal = dir.path().join("receipts");
            let receipt = FileTasks::new(journal.clone())
                .execute(
                    action,
                    (action != FileAction::CreateFolder).then_some(source.as_path()),
                    &destination,
                )
                .unwrap();
            assert_eq!(receipt.state, ReceiptState::Completed);
            let undone = FileTasks::new(journal).undo_last().unwrap();
            assert_eq!(undone.state, ReceiptState::Undone);
            assert!(!destination.exists());
            assert_eq!(fs::read(source).unwrap(), b"verified content");
        }
    }
    #[test]
    fn undo_refuses_changed_payload_occupied_original_and_nonempty_folder() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        let tasks = FileTasks::new(dir.path().join("journal"));
        fs::write(&source, b"original").unwrap();
        tasks
            .execute(FileAction::Move, Some(&source), &destination)
            .unwrap();
        fs::write(&source, b"new user file").unwrap();
        assert!(tasks.undo_last().is_err());
        assert_eq!(fs::read(&source).unwrap(), b"new user file");
        fs::remove_file(&source).unwrap();
        fs::write(&destination, b"edited").unwrap();
        assert!(tasks.undo_last().is_err());
        tasks
            .execute(FileAction::CreateFolder, None, &dir.path().join("folder"))
            .unwrap();
        fs::write(dir.path().join("folder/new.txt"), b"keep me").unwrap();
        assert!(tasks.undo_last().is_err());
        assert!(dir.path().join("folder/new.txt").exists());
    }
    #[test]
    fn copy_undo_preserves_the_last_copy_and_receipt_tampering_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        let tasks = FileTasks::new(dir.path().join("journal"));
        fs::write(&source, b"original").unwrap();
        let receipt = tasks
            .execute(FileAction::Copy, Some(&source), &destination)
            .unwrap();
        fs::remove_file(&source).unwrap();
        assert!(tasks.undo_last().is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"original");
        fs::write(tasks.root.join(format!("{}.dpapi", receipt.id)), b"forged").unwrap();
        assert!(tasks.receipts().is_err());
    }
}
