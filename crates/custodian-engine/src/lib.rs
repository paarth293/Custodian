use custodian_store::dao::TransactionDao;
use custodian_journal::batcher::JournalBatcher;
use custodian_digest::digest_file;
use custodian_winsec::SafePath;
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct EngineCore<'a> {
    dao: TransactionDao<'a>,
    batcher: JournalBatcher,
    root: PathBuf,
}

impl<'a> EngineCore<'a> {
    pub fn new(dao: TransactionDao<'a>, root: PathBuf) -> Self {
        Self {
            dao,
            batcher: JournalBatcher::new(2000),
            root,
        }
    }

    pub async fn run(&self, mut rx: mpsc::Receiver<(PathBuf, notify::EventKind)>) {
        loop {
            let paths = self.batcher.process(&mut rx).await;
            if paths.is_empty() {
                break; // channel closed
            }

            for path in paths {
                if let Ok(safe_path) = SafePath::new(&path, &self.root) {
                    if let Ok(hash) = digest_file(&safe_path) {
                        let id = Uuid::new_v4();
                        let path_str = path.to_string_lossy().to_string();
                        let _ = self.dao.insert_tx(
                            id,
                            "modify",
                            &[path_str],
                            None,
                            Some(&hash)
                        ).await;
                    }
                }
            }
        }
    }
}
