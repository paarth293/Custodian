use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use tokio::sync::mpsc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),
}

pub struct VfsWatcher {
    _watcher: RecommendedWatcher,
}

impl VfsWatcher {
    pub fn new(
        watch_path: PathBuf,
        tx: mpsc::Sender<(PathBuf, EventKind)>,
    ) -> Result<Self, WatcherError> {
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let paths = event.paths.clone();
                let kind = event.kind;
                for path in paths {
                    let _ = tx.blocking_send((path, kind));
                }
            }
        })?;

        watcher.watch(&watch_path, RecursiveMode::Recursive)?;

        Ok(Self { _watcher: watcher })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_watcher() {
        let temp_dir = std::env::temp_dir().join("custodian_watch_test");
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let (tx, mut rx) = mpsc::channel(100);
        let _watcher = VfsWatcher::new(temp_dir.clone(), tx).unwrap();
        
        sleep(Duration::from_millis(200)).await;
        
        let file_path = temp_dir.join("test.txt");
        File::create(&file_path).unwrap();
        
        let event = tokio::time::timeout(Duration::from_millis(1000), rx.recv()).await;
        assert!(event.is_ok(), "Watcher did not receive event in time");
        let (path, _kind) = event.unwrap().unwrap();
        assert_eq!(path, file_path);
    }
}
