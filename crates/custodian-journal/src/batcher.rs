use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

/// Collapses bursts of file-system events into one set of unique paths.
pub struct JournalBatcher {
    debounce_ms: u64,
}

impl JournalBatcher {
    pub fn new(debounce_ms: u64) -> Self {
        Self { debounce_ms }
    }

    /// Waits for the first event, then keeps collecting until the channel is
    /// quiet for the debounce window. Returns an empty batch once the channel
    /// is closed.
    pub async fn process(
        &self,
        rx: &mut mpsc::Receiver<(PathBuf, notify::EventKind)>,
    ) -> Vec<PathBuf> {
        let Some((first, _)) = rx.recv().await else {
            return Vec::new();
        };
        let mut paths = HashSet::from([first]);

        let window = Duration::from_millis(self.debounce_ms);
        while let Ok(Some((path, _))) = timeout(window, rx.recv()).await {
            paths.insert(path);
        }

        paths.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::EventKind;

    #[tokio::test]
    async fn deduplicates_burst() {
        let (tx, mut rx) = mpsc::channel(100);
        let batcher = JournalBatcher::new(50);
        let path = PathBuf::from("file.txt");

        for _ in 0..5 {
            tx.send((path.clone(), EventKind::Any)).await.unwrap();
        }

        assert_eq!(batcher.process(&mut rx).await, vec![path]);
    }

    #[tokio::test]
    async fn closed_channel_yields_empty_batch() {
        let (tx, mut rx) = mpsc::channel::<(PathBuf, EventKind)>(1);
        drop(tx);
        assert!(JournalBatcher::new(10).process(&mut rx).await.is_empty());
    }
}
