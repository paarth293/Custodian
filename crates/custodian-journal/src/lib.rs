//! File-system watching, event debouncing and the write-ahead journal.

pub mod batcher;
pub mod wal;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),
}

/// Recursively watches a directory and forwards every changed path.
pub struct VfsWatcher {
    _watcher: RecommendedWatcher,
}

impl VfsWatcher {
    pub fn new(
        watch_path: &Path,
        tx: mpsc::Sender<(PathBuf, EventKind)>,
    ) -> Result<Self, WatcherError> {
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                for path in event.paths {
                    let _ = tx.blocking_send((path, event.kind));
                }
            }
        })?;
        watcher.watch(watch_path, RecursiveMode::Recursive)?;
        Ok(Self { _watcher: watcher })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, timeout, Duration};

    #[tokio::test]
    async fn reports_created_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (tx, mut rx) = mpsc::channel(100);
        let _watcher = VfsWatcher::new(&root, tx).unwrap();
        sleep(Duration::from_millis(200)).await;

        let file_path = root.join("test.txt");
        std::fs::write(&file_path, b"x").unwrap();

        let (path, _kind) = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("watcher did not report the event in time")
            .unwrap();
        assert_eq!(path, file_path);
    }
}
