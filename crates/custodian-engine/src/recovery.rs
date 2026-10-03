#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TxState {
    Pending,
    Journaled,
    Executed,
    DbMirrored,
}

pub struct TransactionRecoverer;

impl TransactionRecoverer {
    pub fn recover(state: TxState, idempotent_action: impl FnOnce() -> bool) -> TxState {
        match state {
            TxState::Pending => TxState::Pending, 
            TxState::Journaled => {
                if idempotent_action() {
                    TxState::DbMirrored
                } else {
                    TxState::Journaled
                }
            }
            TxState::Executed => {
                TxState::DbMirrored
            }
            TxState::DbMirrored => TxState::DbMirrored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_journaled() {
        let mut ran = false;
        let action = || { ran = true; true };
        
        let new_state = TransactionRecoverer::recover(TxState::Journaled, action);
        assert!(ran);
        assert_eq!(new_state, TxState::DbMirrored);
    }
    
    #[test]
    fn test_recovery_executed() {
        let mut ran = false;
        let action = || { ran = true; true };
        
        let new_state = TransactionRecoverer::recover(TxState::Executed, action);
        assert!(!ran); 
        assert_eq!(new_state, TxState::DbMirrored);
    }
}
