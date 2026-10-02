use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

pub struct JournalBatcher {
    debounce_ms: u64,
}

impl JournalBatcher {
    pub fn new(debounce_ms: u64) -> Self {
        Self { debounce_ms }
    }

    pub async fn process(
        &self,
        mut rx: mpsc::Receiver<(PathBuf, notify::EventKind)>,
    ) -> Vec<PathBuf> {
        let mut paths = HashSet::new();
        
        if let Some((path, _)) = rx.recv().await {
            paths.insert(path);
        } else {
            return vec![];
        }

        let window = Duration::from_millis(self.debounce_ms);
        loop {
            match timeout(window, rx.recv()).await {
                Ok(Some((path, _))) => {
                    paths.insert(path);
                }
                _ => break,
            }
        }

        paths.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::EventKind;

    #[tokio::test]
    async fn test_debounce() {
        let (tx, rx) = mpsc::channel(100);
        let batcher = JournalBatcher::new(50);
        
        let path = PathBuf::from("file.txt");
        
        for _ in 0..5 {
            tx.send((path.clone(), EventKind::Any)).await.unwrap();
        }
        
        let results = batcher.process(rx).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], path);
    }
}
