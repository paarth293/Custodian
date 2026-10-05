use custodian_approval::{Approval, ApprovalError, PlanHash, Verifier};
use custodian_store::dao::TransactionDao;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use uuid::Uuid;

/// A transaction the UI approved, as it reaches the Engine.
#[derive(Debug, Clone)]
pub struct ApprovedPlan {
    pub tx_id: Uuid,
    pub kind: String,
    pub paths: Vec<String>,
    pub approval: Approval,
}

impl ApprovedPlan {
    /// The digest the approval must sign. The Engine computes it from the
    /// transaction it is about to record, so an approval for one transaction
    /// can never be attached to another.
    pub fn digest(tx_id: &Uuid, kind: &str, paths: &[String]) -> PlanHash {
        fn field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
            hasher.update(&(bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
        let mut hasher = blake3::Hasher::new();
        field(&mut hasher, b"custodian-tx-v1");
        field(&mut hasher, tx_id.as_bytes());
        field(&mut hasher, kind.as_bytes());
        for path in paths {
            field(&mut hasher, path.as_bytes());
        }
        hasher.finalize().into()
    }
}

/// Records approved transactions, refusing any approval that does not verify.
pub struct ApprovalExecutor<'a> {
    dao: TransactionDao<'a>,
    verifier: Verifier,
}

impl<'a> ApprovalExecutor<'a> {
    pub fn new(dao: TransactionDao<'a>, verifier: Verifier) -> Self {
        Self { dao, verifier }
    }

    pub async fn run(&self, mut rx: mpsc::Receiver<ApprovedPlan>) {
        while let Some(plan) = rx.recv().await {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if accept(&self.verifier, &plan, now).is_err() {
                continue;
            }
            let _ = self
                .dao
                .insert_tx(plan.tx_id, &plan.kind, &plan.paths, None, None)
                .await;
        }
    }
}

/// The verification step of [`ApprovalExecutor::run`].
fn accept(verifier: &Verifier, plan: &ApprovedPlan, now_unix: u64) -> Result<(), ApprovalError> {
    let digest = ApprovedPlan::digest(&plan.tx_id, &plan.kind, &plan.paths);
    verifier.verify(&digest, &plan.approval, now_unix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use custodian_approval::ApprovalKey;

    #[test]
    fn accepts_only_the_signed_transaction_once() {
        let key = ApprovalKey::generate();
        let verifier = Verifier::new(key.verifying_key());
        let now = 1_800_000_000;
        let tx_id = Uuid::new_v4();
        let paths = vec!["C:/Users/a/copy.jpg".to_string()];
        let approval = key.sign(&ApprovedPlan::digest(&tx_id, "quarantine", &paths), now, 60);
        let plan = ApprovedPlan {
            tx_id,
            kind: "quarantine".into(),
            paths,
            approval,
        };

        // The same approval on another transaction id, kind or path list fails,
        // and does not use up the nonce.
        let mut other_id = plan.clone();
        other_id.tx_id = Uuid::new_v4();
        let mut other_kind = plan.clone();
        other_kind.kind = "delete".into();
        let mut other_paths = plan.clone();
        other_paths.paths.push("C:/Users/a/thesis.docx".into());
        for forged in [&other_id, &other_kind, &other_paths] {
            assert_eq!(accept(&verifier, forged, now), Err(ApprovalError::Auth));
        }

        assert_eq!(accept(&verifier, &plan, now), Ok(()));
        assert_eq!(
            accept(&verifier, &plan, now),
            Err(ApprovalError::NonceReused)
        );
    }
}
