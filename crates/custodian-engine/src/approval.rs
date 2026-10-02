use custodian_store::dao::TransactionDao;
use custodian_proto::SignedApproval;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct ApprovalExecutor<'a> {
    dao: TransactionDao<'a>,
}

impl<'a> ApprovalExecutor<'a> {
    pub fn new(dao: TransactionDao<'a>) -> Self {
        Self { dao }
    }

    pub async fn run(&self, mut rx: mpsc::Receiver<SignedApproval>) {
        loop {
            if let Some(approval) = rx.recv().await {
                if self.verify_signature(&approval) {
                    let id = match Uuid::parse_str(&approval.id) {
                        Ok(uuid) => uuid,
                        Err(_) => continue,
                    };
                    
                    let _ = self.dao.insert_tx(
                        id,
                        "delete",
                        &[],
                        None,
                        None
                    ).await;
                }
            } else {
                break;
            }
        }
    }

    fn verify_signature(&self, approval: &SignedApproval) -> bool {
        // Mock verification
        !approval.signature.is_empty()
    }
}
