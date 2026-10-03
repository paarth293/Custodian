use custodian_proto::SignedApproval;
use custodian_store::dao::TransactionDao;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Persists approvals received from the UI once their signature is present.
pub struct ApprovalExecutor<'a> {
    dao: TransactionDao<'a>,
}

impl<'a> ApprovalExecutor<'a> {
    pub fn new(dao: TransactionDao<'a>) -> Self {
        Self { dao }
    }

    pub async fn run(&self, mut rx: mpsc::Receiver<SignedApproval>) {
        while let Some(approval) = rx.recv().await {
            if !Self::has_signature(&approval) {
                continue;
            }
            let Ok(id) = Uuid::parse_str(&approval.id) else {
                continue;
            };
            let _ = self.dao.insert_tx(id, "delete", &[], None, None).await;
        }
    }

    /// Placeholder: cryptographic verification lives in
    /// [`crate::skeleton::EngineSkeleton::verify_approval`].
    fn has_signature(approval: &SignedApproval) -> bool {
        !approval.signature.is_empty()
    }
}
