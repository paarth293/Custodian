//! Custodian engine: applies approved, journaled and reversible file operations.

pub mod approval;
pub mod disk;
pub mod fs;
pub mod ops;
pub mod purge;
pub mod recovery;
pub mod skeleton;
pub mod undo;

use custodian_digest::digest_file;
use custodian_journal::batcher::JournalBatcher;
use custodian_store::dao::TransactionDao;
use custodian_winsec::SafePath;
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Debounce window applied to file-system events before they are recorded.
const DEBOUNCE_MS: u64 = 2000;

pub struct EngineCore<'a> {
    dao: TransactionDao<'a>,
    batcher: JournalBatcher,
    root: PathBuf,
}

impl<'a> EngineCore<'a> {
    pub fn new(dao: TransactionDao<'a>, root: PathBuf) -> Self {
        Self {
            dao,
            batcher: JournalBatcher::new(DEBOUNCE_MS),
            root,
        }
    }

    /// Records a `modify` transaction for every debounced path until the
    /// event channel closes.
    pub async fn run(&self, mut rx: mpsc::Receiver<(PathBuf, notify::EventKind)>) {
        loop {
            let paths = self.batcher.process(&mut rx).await;
            if paths.is_empty() {
                break;
            }

            for path in paths {
                let Ok(safe_path) = SafePath::new(&path, &self.root) else {
                    continue;
                };
                let Ok(hash) = digest_file(&safe_path) else {
                    continue;
                };
                let path_str = path.to_string_lossy().into_owned();
                // A failed insert is retried on the next event for this path.
                let _ = self
                    .dao
                    .insert_tx(Uuid::new_v4(), "modify", &[path_str], None, Some(&hash))
                    .await;
            }
        }
    }
}
