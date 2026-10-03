/// Lifecycle of a transaction across the journal, the file system and the DB.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TxState {
    Pending,
    Journaled,
    Executed,
    DbMirrored,
}

pub struct TransactionRecoverer;

impl TransactionRecoverer {
    /// Advances a transaction found during crash recovery.
    ///
    /// `Journaled` transactions re-run their idempotent action; `Executed`
    /// ones only need their DB mirror; `Pending` ones were never committed.
    pub fn recover(state: TxState, idempotent_action: impl FnOnce() -> bool) -> TxState {
        match state {
            TxState::Pending => TxState::Pending,
            TxState::Journaled if idempotent_action() => TxState::DbMirrored,
            TxState::Journaled => TxState::Journaled,
            TxState::Executed | TxState::DbMirrored => TxState::DbMirrored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journaled_reruns_action() {
        let mut ran = false;
        let state = TransactionRecoverer::recover(TxState::Journaled, || {
            ran = true;
            true
        });
        assert!(ran);
        assert_eq!(state, TxState::DbMirrored);
    }

    #[test]
    fn journaled_stays_journaled_on_failure() {
        assert_eq!(
            TransactionRecoverer::recover(TxState::Journaled, || false),
            TxState::Journaled
        );
    }

    #[test]
    fn executed_skips_action() {
        let mut ran = false;
        let state = TransactionRecoverer::recover(TxState::Executed, || {
            ran = true;
            true
        });
        assert!(!ran);
        assert_eq!(state, TxState::DbMirrored);
    }
}
