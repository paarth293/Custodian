use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Error, PartialEq)]
pub enum ApprovalError {
    #[error("E_AUTH")]
    Auth,
    #[error("E_EXPIRED")]
    Expired,
    #[error("E_NONCE")]
    NonceReused,
    #[error("E_STALE")]
    StaleFacts,
}

/// The bytes an approver must sign: the plan hash plus the expiry and nonce.
///
/// Signing the plan hash alone would let anyone who has seen a valid approval
/// re-submit it with a fresh nonce and a later expiry, defeating both replay
/// protection and expiry.
pub fn approval_message(plan_hash: &[u8], expiry_ts: u64, nonce: &str) -> Vec<u8> {
    let mut msg = Vec::with_capacity(plan_hash.len() + 8 + nonce.len() + 8);
    msg.extend_from_slice(&(plan_hash.len() as u64).to_le_bytes());
    msg.extend_from_slice(plan_hash);
    msg.extend_from_slice(&expiry_ts.to_le_bytes());
    msg.extend_from_slice(nonce.as_bytes());
    msg
}

pub struct EngineSkeleton {
    public_key: VerifyingKey,
    used_nonces: Mutex<HashSet<String>>,
}

impl EngineSkeleton {
    pub fn new(public_key: VerifyingKey) -> Self {
        Self {
            public_key,
            used_nonces: Mutex::new(HashSet::new()),
        }
    }

    pub fn verify_approval(
        &self,
        plan_hash: &[u8],
        signature_bytes: &[u8; 64],
        expiry_ts: u64,
        nonce: &str,
    ) -> Result<(), ApprovalError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if now > expiry_ts {
            return Err(ApprovalError::Expired);
        }

        let mut nonces = self.used_nonces.lock().unwrap();
        if nonces.contains(nonce) {
            return Err(ApprovalError::NonceReused);
        }

        let signature = Signature::from_bytes(signature_bytes);
        let message = approval_message(plan_hash, expiry_ts, nonce);
        if self.public_key.verify_strict(&message, &signature).is_err() {
            return Err(ApprovalError::Auth);
        }

        nonces.insert(nonce.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{SigningKey, Signer};
    use rand::rngs::OsRng;

    #[test]
    fn test_t_j2_approval_verification() {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let public_key = signing_key.verifying_key();
        
        let engine = EngineSkeleton::new(public_key);
        
        let plan_hash = b"valid_plan_hash";
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let valid_expiry = now + 600; // 10 mins
        let sign = |plan: &[u8], expiry: u64, nonce: &str| {
            signing_key
                .sign(&approval_message(plan, expiry, nonce))
                .to_bytes()
        };
        let signature = sign(plan_hash, valid_expiry, "nonce1");

        // 1. Accepted, then no-op (nonce recorded)
        assert_eq!(
            engine.verify_approval(plan_hash, &signature, valid_expiry, "nonce1"),
            Ok(())
        );

        // 2. Reused nonce -> E_NONCE
        assert_eq!(
            engine.verify_approval(plan_hash, &signature, valid_expiry, "nonce1"),
            Err(ApprovalError::NonceReused)
        );

        // 3. Tampered plan -> E_AUTH
        assert_eq!(
            engine.verify_approval(b"tampered", &signature, valid_expiry, "nonce2"),
            Err(ApprovalError::Auth)
        );

        // 3b. Replaying a seen signature under a fresh nonce or a later expiry -> E_AUTH
        assert_eq!(
            engine.verify_approval(plan_hash, &signature, valid_expiry, "fresh-nonce"),
            Err(ApprovalError::Auth)
        );
        assert_eq!(
            engine.verify_approval(plan_hash, &signature, valid_expiry + 3600, "nonce1b"),
            Err(ApprovalError::Auth)
        );

        // 4. Expired approval -> E_EXPIRED
        assert_eq!(
            engine.verify_approval(plan_hash, &sign(plan_hash, now - 10, "nonce3"), now - 10, "nonce3"),
            Err(ApprovalError::Expired)
        );
    }
}
