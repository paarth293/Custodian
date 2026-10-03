#[derive(Clone, Debug, PartialEq)]
pub struct TxRecord {
    pub id: String,
    pub path: String,
    pub timestamp: u64,
}

pub struct UndoManager {
    history: Vec<TxRecord>,
}

impl UndoManager {
    pub fn new(history: Vec<TxRecord>) -> Self {
        Self { history }
    }

    pub fn can_undo(&self, tx_id: &str) -> Result<(), &'static str> {
        let target_idx = self.history.iter().position(|tx| tx.id == tx_id)
            .ok_or("Transaction not found")?;
            
        let target_path = &self.history[target_idx].path;
        
        for i in (target_idx + 1)..self.history.len() {
            if &self.history[i].path == target_path {
                return Err("Conflict: File modified by newer transaction");
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_rules() {
        let history = vec![
            TxRecord { id: "tx1".into(), path: "C:\\fileA.txt".into(), timestamp: 1 },
            TxRecord { id: "tx2".into(), path: "C:\\fileB.txt".into(), timestamp: 2 },
            TxRecord { id: "tx3".into(), path: "C:\\fileA.txt".into(), timestamp: 3 },
        ];
        let manager = UndoManager::new(history);
        
        assert!(manager.can_undo("tx3").is_ok());
        assert!(manager.can_undo("tx2").is_ok());
        assert!(manager.can_undo("tx1").is_err());
    }
}
