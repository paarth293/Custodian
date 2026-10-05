//! The Engine's approval gate (build plan step 56, T-J2): nothing runs unless
//! it carries an approval signed over exactly this plan, unexpired and unused.

pub use custodian_approval::ApprovalError;
use custodian_approval::{Approval, PlanHash, Verifier};
use ed25519_dalek::VerifyingKey;

pub struct EngineSkeleton {
    verifier: Verifier,
}

impl EngineSkeleton {
    pub fn new(public_key: VerifyingKey) -> Self {
        Self {
            verifier: Verifier::new(public_key),
        }
    }

    pub fn verify_approval(
        &self,
        plan_hash: &PlanHash,
        approval: &Approval,
        now_unix: u64,
    ) -> Result<(), ApprovalError> {
        self.verifier.verify(plan_hash, approval, now_unix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use custodian_approval::ApprovalKey;

    #[test]
    fn test_t_j2_approval_verification() {
        let key = ApprovalKey::generate();
        let engine = EngineSkeleton::new(key.verifying_key());
        let plan = [1u8; 32];
        let now = 1_800_000_000;
        let approval = key.sign(&plan, now, 600);

        // Accepted once, then the nonce is spent.
        assert_eq!(engine.verify_approval(&plan, &approval, now), Ok(()));
        assert_eq!(
            engine.verify_approval(&plan, &approval, now),
            Err(ApprovalError::NonceReused)
        );
        // A tampered plan, and an expired approval.
        let fresh = key.sign(&plan, now, 600);
        assert_eq!(
            engine.verify_approval(&[2u8; 32], &fresh, now),
            Err(ApprovalError::Auth)
        );
        assert_eq!(
            engine.verify_approval(&plan, &fresh, now + 601),
            Err(ApprovalError::Expired)
        );
    }
}
