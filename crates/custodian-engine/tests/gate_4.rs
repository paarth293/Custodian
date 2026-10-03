use custodian_engine::recovery::{TransactionRecoverer, TxState};

#[test]
fn test_gate_4_recovery_without_db() {
    let journaled_transactions = vec![TxState::Journaled, TxState::Pending];

    let mut db_mirrored = 0;

    for state in journaled_transactions {
        let new_state = TransactionRecoverer::recover(state, || true);

        if new_state == TxState::DbMirrored {
            db_mirrored += 1;
        }
    }

    assert_eq!(db_mirrored, 1);
}
