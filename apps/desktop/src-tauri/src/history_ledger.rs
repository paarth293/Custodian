//! Records clean-ups and undos in the tamper-evident ledger (A-09.8).
//!
//! Before each append the ledger is checked against its sealed anchor, so an
//! edited or shortened history is noticed, recorded as a security event and
//! reported to the person.

use custodian_ledger::anchor::AnchorManager;
use custodian_ledger::chain::Ledger;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Mutex;

/// One writer at a time: two appends that both start from the same head
/// would break the chain for good.
static WRITER: Mutex<()> = Mutex::new(());

/// The state of the history before the new record was added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerStatus {
    Intact,
    TamperDetected,
}

/// Appends `event` to the ledger in `dir` and moves the anchor forward.
pub fn record(dir: &Path, event: &Value) -> Result<LedgerStatus, String> {
    let _writer = WRITER.lock().unwrap_or_else(|e| e.into_inner());
    let fail = |e: &dyn std::fmt::Display| format!("I couldn't update my history log: {e}");
    std::fs::create_dir_all(dir).map_err(|e| fail(&e))?;
    let anchors = AnchorManager::new(dir.join("anchor.dpapi"));
    let mut ledger = Ledger::new(dir.join("ledger.bin")).map_err(|e| fail(&e))?;

    // A torn final record (`Ledger::new` drops it) is what a crash during an
    // append leaves; the anchor is only saved after a complete append, so it
    // still matches and this is not reported as tampering.
    let intact = match anchors.load() {
        Ok(Some(anchor)) => ledger.verify_anchor(&anchor).is_ok(),
        // No anchor yet is fine only for a brand-new, empty ledger.
        Ok(None) => ledger.height() == 0,
        Err(_) => false,
    };

    if !intact {
        let alert = json!({ "kind": "security", "event": "ledger_tamper_detected" });
        ledger
            .append_record(alert.to_string().as_bytes())
            .map_err(|e| fail(&e))?;
    }
    ledger
        .append_record(event.to_string().as_bytes())
        .map_err(|e| fail(&e))?;
    anchors.save(&ledger.anchor()).map_err(|e| fail(&e))?;

    Ok(if intact {
        LedgerStatus::Intact
    } else {
        LedgerStatus::TamperDetected
    })
}

/// Checks the history against its sealed anchor without adding to it.
pub fn check(dir: &Path) -> Result<LedgerStatus, String> {
    let _writer = WRITER.lock().unwrap_or_else(|e| e.into_inner());
    let path = dir.join("ledger.bin");
    let anchors = AnchorManager::new(dir.join("anchor.dpapi"));
    if !path.exists() {
        // F11: If the ledger is missing, it is only intact if there is also NO anchor.
        // If an anchor exists (or is corrupted), the ledger was removed or tampered.
        return match anchors.load() {
            Ok(None) => Ok(LedgerStatus::Intact),
            _ => Ok(LedgerStatus::TamperDetected),
        };
    }
    let mut ledger = Ledger::new(path).map_err(|e| e.to_string())?;
    let intact = match anchors.load() {
        Ok(Some(anchor)) => ledger.verify_anchor(&anchor).is_ok(),
        Ok(None) => ledger.height() == 0,
        Err(_) => false,
    };
    Ok(if intact {
        LedgerStatus::Intact
    } else {
        LedgerStatus::TamperDetected
    })
}

/// A sentence to add to Cubby's reply when the history needs attention.
pub fn note(result: &Result<LedgerStatus, String>) -> Option<String> {
    match result {
        Ok(LedgerStatus::Intact) => None,
        Ok(LedgerStatus::TamperDetected) => Some(
            "Heads up: my history log was changed outside Custodian since my last entry. I've recorded that, and kept going."
                .into(),
        ),
        Err(err) => Some(err.clone()),
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn first_live_record_creates_its_history_directory() {
        let root = tempfile::tempdir().unwrap();
        let history = root.path().join("new-profile").join("ledger");
        assert!(!history.exists());
        assert_eq!(
            record(&history, &json!({"kind": "fixture"})).unwrap(),
            LedgerStatus::Intact
        );
        assert!(history.join("ledger.bin").is_file());
        assert!(history.join("anchor.dpapi").is_file());
        assert_eq!(check(&history).unwrap(), LedgerStatus::Intact);
    }
    use std::fs::{self, OpenOptions};

    #[test]
    fn records_events_and_notices_a_cut_history() {
        let dir = tempfile::tempdir().unwrap();
        let event = |n: u32| json!({ "kind": "cleanup", "plan": n });

        assert_eq!(record(dir.path(), &event(1)), Ok(LedgerStatus::Intact));
        let after_one = fs::metadata(dir.path().join("ledger.bin")).unwrap().len();
        assert_eq!(record(dir.path(), &event(2)), Ok(LedgerStatus::Intact));

        // Someone deletes the newest entry; the chain alone would still verify.
        OpenOptions::new()
            .write(true)
            .open(dir.path().join("ledger.bin"))
            .unwrap()
            .set_len(after_one)
            .unwrap();
        let status = record(dir.path(), &event(3));
        assert_eq!(status, Ok(LedgerStatus::TamperDetected));
        assert!(note(&status).unwrap().contains("changed outside Custodian"));

        // The alert itself is now part of the history, and the log is consistent again.
        let mut ledger = Ledger::new(dir.path().join("ledger.bin")).unwrap();
        assert_eq!(ledger.height(), 3);
        ledger.verify().unwrap();
        assert_eq!(record(dir.path(), &event(4)), Ok(LedgerStatus::Intact));
    }

    #[test]
    fn checks_without_adding_anything() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(check(dir.path()), Ok(LedgerStatus::Intact), "nothing yet");
        record(dir.path(), &json!({ "kind": "cleanup" })).unwrap();
        let size = fs::metadata(dir.path().join("ledger.bin")).unwrap().len();
        assert_eq!(check(dir.path()), Ok(LedgerStatus::Intact));
        assert_eq!(
            fs::metadata(dir.path().join("ledger.bin")).unwrap().len(),
            size
        );
        fs::remove_file(dir.path().join("anchor.dpapi")).unwrap();
        assert_eq!(check(dir.path()), Ok(LedgerStatus::TamperDetected));
    }

    #[test]
    fn a_deleted_anchor_is_noticed() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), &json!({ "kind": "undo" })).unwrap();
        fs::remove_file(dir.path().join("anchor.dpapi")).unwrap();
        assert_eq!(
            record(dir.path(), &json!({ "kind": "undo" })),
            Ok(LedgerStatus::TamperDetected)
        );
    }

    #[test]
    fn a_deleted_ledger_with_existing_anchor_is_noticed() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), &json!({ "kind": "cleanup" })).unwrap();
        // Attacker deletes ledger.bin but anchor.dpapi remains
        fs::remove_file(dir.path().join("ledger.bin")).unwrap();
        assert_eq!(check(dir.path()), Ok(LedgerStatus::TamperDetected));
    }

    #[test]
    fn concurrent_records_keep_the_chain_intact() {
        let dir = tempfile::tempdir().unwrap();
        let threads: Vec<_> = (0..8)
            .map(|i| {
                let dir = dir.path().to_path_buf();
                std::thread::spawn(move || record(&dir, &json!({ "kind": "cleanup", "n": i })))
            })
            .collect();
        for t in threads {
            assert_eq!(t.join().unwrap(), Ok(LedgerStatus::Intact));
        }
        let mut ledger = Ledger::new(dir.path().join("ledger.bin")).unwrap();
        assert_eq!(ledger.height(), 8);
        ledger.verify().unwrap();
    }

    #[test]
    fn a_crash_mid_append_is_not_called_tampering() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), &json!({ "kind": "cleanup" })).unwrap();
        // Half a record, as a power cut during the next append would leave.
        OpenOptions::new()
            .append(true)
            .open(dir.path().join("ledger.bin"))
            .unwrap()
            .write_all(&[0xAB; 20])
            .unwrap();
        assert_eq!(
            record(dir.path(), &json!({ "kind": "undo" })),
            Ok(LedgerStatus::Intact)
        );
        Ledger::new(dir.path().join("ledger.bin"))
            .unwrap()
            .verify()
            .unwrap();
    }
}
