//! Encrypted append-only application effects journal. Loading evidence never
//! grants authority and never replays application input after a restart.
use crate::{
    app_adapter::{ApplicationReceipt, ApplicationRequest},
    app_worker::PendingDispatch,
    task_program::{Failure, FailureClass},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
const LIMIT: u64 = 512 * 1024;
const MAX_RECORDS: usize = 256;
const TOTAL_LIMIT: u64 = 8 * 1024 * 1024;
fn invalid() -> io::Error {
    io::Error::other("Application recovery evidence unavailable; no replay permitted")
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Prepared {
        plan_digest: [u8; 32],
    },
    Scope {
        scope: RecoveryScope,
    },
    StagingPending {
        source_digest: [u8; 32],
    },
    Staged {
        source_digest: [u8; 32],
    },
    LaunchPending {
        executable_digest: [u8; 32],
    },
    OwnedProcess {
        pid: u32,
        birth: u64,
    },
    Pending {
        nonce: String,
        request_digest: [u8; 32],
    },
    Receipt {
        request_digest: [u8; 32],
        receipt: ApplicationReceipt,
    },
    OutputVerified {
        digest: [u8; 32],
        bytes: u64,
    },
    PublishPending {
        receipt_id: String,
    },
    Published {
        receipt_id: String,
    },
    Completed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryScope {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub source_identity: custodian_winsec::fsops::FileIdentity,
    pub source_digest: [u8; 32],
    pub expected: ExpectedArtifact,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExpectedArtifact {
    Text { digest: [u8; 32], bytes: u64 },
    Workbook { snapshot_digest: [u8; 32] },
}
impl ExpectedArtifact {
    pub fn matches(&self, bytes: &[u8]) -> bool {
        match self {
            Self::Text {
                digest,
                bytes: count,
            } => bytes.len() as u64 == *count && blake3::hash(bytes).as_bytes() == digest,
            Self::Workbook { snapshot_digest } => {
                crate::workbook::inspect(bytes)
                    .ok()
                    .is_some_and(|snapshot| {
                        crate::workbook::recalculate(&snapshot)
                            .is_ok_and(|calculated| calculated == snapshot)
                            && serde_json::to_vec(&snapshot)
                                .is_ok_and(|data| blake3::hash(&data).as_bytes() == snapshot_digest)
                    })
            }
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u32,
    journal: String,
    sequence: usize,
    previous: [u8; 32],
    event: Event,
}
#[derive(Debug, Clone)]
pub struct RecoveryEvidence {
    pub events: Vec<Event>,
    pub completed: bool,
    pub uncertain_effect: bool,
}
struct State {
    root: PathBuf,
    binding: String,
    previous: [u8; 32],
    events: Vec<Event>,
    pending: Option<[u8; 32]>,
    poisoned: bool,
    sealed_bytes: u64,
    _lock: File,
    _directory: custodian_winsec::guarded_copy::DirectoryGuard,
}
#[derive(Clone)]
pub struct ApplicationJournal(Arc<Mutex<State>>);
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
fn lock(root: &Path) -> io::Result<File> {
    if !root.is_absolute() {
        return Err(invalid());
    }
    for ancestor in root.ancestors() {
        regular(ancestor)?;
    }
    let path = root.join("owner.lock");
    if path.try_exists()? {
        regular(&path)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    options.open(path)
}
fn transition(
    events: &[Event],
    event: &Event,
    pending: Option<[u8; 32]>,
) -> io::Result<Option<[u8; 32]>> {
    if matches!(events.last(), Some(Event::Completed)) {
        return Err(invalid());
    }
    match event {
        Event::Prepared { .. } if events.is_empty() => Ok(None),
        Event::Scope { scope }
            if matches!(events.last(), Some(Event::Prepared { .. }))
                && scope.source.is_absolute()
                && scope.destination.is_absolute()
                && scope.source != scope.destination =>
        {
            Ok(None)
        }
        Event::StagingPending { source_digest }
            if matches!(
                events.last(),
                Some(Event::Prepared { .. } | Event::Scope { .. })
            ) =>
        {
            Ok(Some(*source_digest))
        }
        Event::Staged { source_digest } if matches!(events.last(), Some(Event::StagingPending { source_digest: expected }) if expected == source_digest) => {
            Ok(None)
        }
        Event::LaunchPending { executable_digest }
            if matches!(
                events.last(),
                Some(Event::Prepared { .. } | Event::Staged { .. })
            ) =>
        {
            Ok(Some(*executable_digest))
        }
        Event::OwnedProcess { pid, birth }
            if matches!(events.last(), Some(Event::LaunchPending { .. }))
                && *pid != 0
                && *birth != 0 =>
        {
            Ok(None)
        }
        Event::Pending {
            nonce,
            request_digest,
        } if !events.is_empty() && pending.is_none() && uuid::Uuid::parse_str(nonce).is_ok() => {
            Ok(Some(*request_digest))
        }
        Event::Receipt {
            request_digest,
            receipt,
        } if pending == Some(*request_digest) && receipt.verified && !receipt.detail.is_empty() => {
            Ok(None)
        }
        Event::OutputVerified { bytes, .. }
            if !events.is_empty() && pending.is_none() && *bytes <= 8 * 1024 * 1024 =>
        {
            Ok(None)
        }
        Event::PublishPending { receipt_id }
            if matches!(events.last(), Some(Event::OutputVerified { .. }))
                && !receipt_id.is_empty()
                && receipt_id.len() <= 128
                && !receipt_id.chars().any(char::is_control) =>
        {
            Ok(None)
        }
        Event::Published { receipt_id } if matches!(events.last(), Some(Event::PublishPending { receipt_id: pending }) if pending == receipt_id) => {
            Ok(None)
        }
        Event::Completed
            if matches!(events.last(), Some(Event::Published { .. })) && pending.is_none() =>
        {
            Ok(None)
        }
        _ => Err(invalid()),
    }
}
impl ApplicationJournal {
    /// Native caller chooses a new task-owned directory. Existing evidence is
    /// never overwritten or reset by this constructor.
    pub fn create(root: &Path, plan_digest: [u8; 32]) -> io::Result<Self> {
        if !root.is_absolute() {
            return Err(invalid());
        }
        for ancestor in root.parent().ok_or_else(invalid)?.ancestors() {
            regular(ancestor)?;
        }
        fs::create_dir(root)?;
        let directory = custodian_winsec::guarded_copy::DirectoryGuard::open(root)?;
        let owned = lock(root)?;
        let root = root.canonicalize()?;
        let binding = blake3::hash(root.to_string_lossy().as_bytes())
            .to_hex()
            .to_string();
        let result = Self(Arc::new(Mutex::new(State {
            root,
            binding,
            previous: [0; 32],
            events: vec![],
            pending: None,
            poisoned: false,
            sealed_bytes: 0,
            _lock: owned,
            _directory: directory,
        })));
        result.append(Event::Prepared { plan_digest })?;
        Ok(result)
    }
    /// Returns evidence only. No resume action, native grant or replay is
    /// produced, even when the last recorded receipt was successful.
    pub fn recover(root: &Path) -> io::Result<RecoveryEvidence> {
        let _directory = custodian_winsec::guarded_copy::DirectoryGuard::open(root)?;
        let _lock = lock(root)?;
        let root = root.canonicalize()?;
        let binding = blake3::hash(root.to_string_lossy().as_bytes())
            .to_hex()
            .to_string();
        let mut paths = fs::read_dir(&root)?
            .take(MAX_RECORDS + 2)
            .map(|entry| entry.map(|e| e.path()))
            .collect::<io::Result<Vec<_>>>()?;
        paths.retain(|p| p.file_name().and_then(|n| n.to_str()) != Some("owner.lock"));
        paths.sort();
        if paths.is_empty() || paths.len() > MAX_RECORDS {
            return Err(invalid());
        }
        let mut previous = [0; 32];
        let mut events = vec![];
        let mut pending = None;
        let mut total = 0u64;
        for (sequence, path) in paths.iter().enumerate() {
            if path.file_name().and_then(|n| n.to_str()) != Some(&format!("{sequence:04}.dpapi")) {
                return Err(invalid());
            }
            regular(path)?;
            let mut bytes = vec![];
            File::open(path)?.take(LIMIT + 1).read_to_end(&mut bytes)?;
            total = total.checked_add(bytes.len() as u64).ok_or_else(invalid)?;
            if bytes.len() as u64 > LIMIT || total > TOTAL_LIMIT {
                return Err(invalid());
            }
            let plain = custodian_winsec::dpapi::unprotect(&bytes)?;
            let record: Record = serde_json::from_slice(&plain).map_err(|_| invalid())?;
            if record.schema != 1
                || record.journal != binding
                || record.sequence != sequence
                || record.previous != previous
            {
                return Err(invalid());
            }
            pending = transition(&events, &record.event, pending)?;
            previous = *blake3::hash(&plain).as_bytes();
            events.push(record.event);
        }
        let completed = matches!(events.last(), Some(Event::Completed));
        let uncertain_effect =
            pending.is_some() || matches!(events.last(), Some(Event::PublishPending { .. }));
        Ok(RecoveryEvidence {
            events,
            completed,
            uncertain_effect,
        })
    }
    pub fn append(&self, event: Event) -> io::Result<()> {
        let mut state = self.0.lock().map_err(|_| invalid())?;
        if state.poisoned || state.events.len() >= MAX_RECORDS {
            return Err(invalid());
        }
        let pending = transition(&state.events, &event, state.pending)?;
        let record = Record {
            schema: 1,
            journal: state.binding.clone(),
            sequence: state.events.len(),
            previous: state.previous,
            event,
        };
        let bytes = serde_json::to_vec(&record).map_err(|_| invalid())?;
        if bytes.len() as u64 > LIMIT - 4096 {
            return Err(invalid());
        }
        let sealed = custodian_winsec::dpapi::protect(&bytes)?;
        let total = state
            .sealed_bytes
            .checked_add(sealed.len() as u64)
            .ok_or_else(invalid)?;
        if sealed.len() as u64 > LIMIT || total > TOTAL_LIMIT {
            return Err(invalid());
        }
        let save = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(state.root.join(format!("{:04}.dpapi", state.events.len())))?;
            file.write_all(&sealed)?;
            file.sync_all()
        })();
        if save.is_err() {
            state.poisoned = true;
            return Err(invalid());
        }
        state.previous = *blake3::hash(&bytes).as_bytes();
        state.sealed_bytes = total;
        state.pending = pending;
        state.events.push(record.event);
        Ok(())
    }
    pub fn record_receipt(
        &self,
        request: &ApplicationRequest,
        receipt: ApplicationReceipt,
    ) -> io::Result<()> {
        if request.identity != receipt.identity
            || request.document_id != receipt.document_id
            || request.document_name != receipt.document_name
            || request.action != receipt.action
        {
            return Err(invalid());
        }
        let digest = *blake3::hash(&serde_json::to_vec(request).map_err(|_| invalid())?).as_bytes();
        self.append(Event::Receipt {
            request_digest: digest,
            receipt,
        })
    }
}
impl PendingDispatch for ApplicationJournal {
    fn record_pending(&mut self, nonce: &str, request_digest: [u8; 32]) -> Result<(), Failure> {
        self.append(Event::Pending {
            nonce: nonce.into(),
            request_digest,
        })
        .map_err(|_| {
            Failure::new(
                FailureClass::Unrecoverable,
                "Application effect checkpoint failed; input was refused",
            )
        })
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn interrupted_dispatch_is_uncertain_and_corrupt_or_missing_evidence_is_refused() {
        for fault in ["none", "corrupt", "gap", "moved"] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("journal");
            let mut journal = ApplicationJournal::create(&root, [3; 32]).unwrap();
            journal
                .record_pending(&uuid::Uuid::new_v4().to_string(), [7; 32])
                .unwrap();
            assert!(
                ApplicationJournal::recover(&root).is_err(),
                "live owner was bypassed"
            );
            assert!(journal.append(Event::Completed).is_err());
            drop(journal);
            let path = match fault {
                "corrupt" => {
                    fs::write(root.join("0001.dpapi"), b"not valid evidence").unwrap();
                    root
                }
                "gap" => {
                    fs::remove_file(root.join("0000.dpapi")).unwrap();
                    root
                }
                "moved" => {
                    let other = temp.path().join("other");
                    fs::rename(&root, &other).unwrap();
                    other
                }
                _ => root,
            };
            let result = ApplicationJournal::recover(&path);
            if fault == "none" {
                let evidence = result.unwrap();
                assert!(evidence.uncertain_effect);
                assert!(!evidence.completed);
            } else {
                assert!(result.is_err(), "accepted damaged evidence: {fault}");
            }
        }
    }
}
