#[derive(Clone, Debug, PartialEq)]
pub struct TxRecord {
    pub id: String,
    pub path: String,
    pub timestamp: u64,
}

/// Decides whether a transaction can be undone without clobbering newer work.
pub struct UndoManager {
    /// Transactions in chronological order.
    history: Vec<TxRecord>,
}

impl UndoManager {
    pub fn new(history: Vec<TxRecord>) -> Self {
        Self { history }
    }

    pub fn can_undo(&self, tx_id: &str) -> Result<(), &'static str> {
        let target_idx = self
            .history
            .iter()
            .position(|tx| tx.id == tx_id)
            .ok_or("Transaction not found")?;
        let target_path = &self.history[target_idx].path;

        if self.history[target_idx + 1..]
            .iter()
            .any(|tx| &tx.path == target_path)
        {
            return Err("Conflict: File modified by newer transaction");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tx(id: &str, path: &str, timestamp: u64) -> TxRecord {
        TxRecord {
            id: id.into(),
            path: path.into(),
            timestamp,
        }
    }

    #[test]
    fn blocks_undo_when_newer_tx_touches_same_path() {
        let manager = UndoManager::new(vec![
            tx("tx1", r"C:\fileA.txt", 1),
            tx("tx2", r"C:\fileB.txt", 2),
            tx("tx3", r"C:\fileA.txt", 3),
        ]);

        assert!(manager.can_undo("tx3").is_ok());
        assert!(manager.can_undo("tx2").is_ok());
        assert!(manager.can_undo("tx1").is_err());
        assert!(manager.can_undo("missing").is_err());
    }
}
