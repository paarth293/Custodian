//! Signed approvals (A-09.1, SR-D1).
//!
//! An [`Approval`] is an Ed25519 signature over a plan's hash, an expiry and a
//! random nonce, so it authorises exactly one plan, once, for a few minutes.
//! The signing key never leaves the process that shows the approval prompt;
//! it is stored on disk only under DPAPI ([`ApprovalKey::load_or_create`]).

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;
use zeroize::Zeroizing;

/// Longest an approval may stay valid (build plan step 56).
pub const MAX_LIFETIME_SECS: u64 = 600;

const DOMAIN: &[u8] = b"custodian-approval-v1\0";

/// A 32-byte digest identifying the exact plan being approved.
pub type PlanHash = [u8; 32];

/// Why an approval was refused.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum ApprovalError {
    #[error("E_AUTH: the approval signature does not match this plan")]
    Auth,
    #[error("E_EXPIRED: the approval has expired")]
    Expired,
    #[error("E_AUTH: the approval is valid for longer than allowed")]
    LifetimeTooLong,
    #[error("E_NONCE: the approval was already used")]
    NonceReused,
}

/// A signed authorisation to run one plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    pub expires_unix: u64,
    pub nonce: [u8; 16],
    pub signature: [u8; 64],
}

/// The bytes that are signed: every field an attacker might want to change.
fn message(plan_hash: &PlanHash, expires_unix: u64, nonce: &[u8; 16]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(DOMAIN.len() + 32 + 8 + 16);
    msg.extend_from_slice(DOMAIN);
    msg.extend_from_slice(plan_hash);
    msg.extend_from_slice(&expires_unix.to_le_bytes());
    msg.extend_from_slice(nonce);
    msg
}

/// The approval signing key. Zeroised in memory when dropped.
pub struct ApprovalKey(SigningKey);

impl ApprovalKey {
    pub fn generate() -> Self {
        Self(SigningKey::generate(&mut OsRng))
    }

    /// Loads the key sealed with DPAPI at `path`, creating it on first run.
    /// The plaintext key is never written to disk.
    ///
    /// A key file that cannot be unsealed (cut short by a crash, or sealed
    /// under DPAPI keys that no longer exist) is set aside as `*.unreadable`
    /// and replaced. That is safe because the key is only ever checked by this
    /// same process: a new key simply means approvals signed before now stop
    /// verifying, and they expire within minutes anyway.
    pub fn load_or_create(path: &Path) -> io::Result<Self> {
        match fs::read(path) {
            Ok(sealed) => match Self::unseal(&sealed) {
                Some(key) => Ok(key),
                None => {
                    fs::rename(path, path.with_extension("unreadable"))?;
                    Self::create(path)
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => Self::create(path),
            Err(err) => Err(err),
        }
    }

    fn unseal(sealed: &[u8]) -> Option<Self> {
        let bytes = Zeroizing::new(custodian_winsec::dpapi::unprotect(sealed).ok()?);
        let secret: &[u8; 32] = bytes.as_slice().try_into().ok()?;
        Some(Self(SigningKey::from_bytes(secret)))
    }

    fn create(path: &Path) -> io::Result<Self> {
        let key = Self::generate();
        let secret = Zeroizing::new(key.0.to_bytes());
        let sealed = custodian_winsec::dpapi::protect(secret.as_slice())?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        // Written and flushed in full before it takes the real name, so a
        // crash never leaves a partial key file behind.
        let tmp = path.with_extension("tmp");
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&sealed)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)?;
        Ok(key)
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.0.verifying_key()
    }

    /// Approves `plan_hash` until `now_unix + lifetime_secs`.
    pub fn sign(&self, plan_hash: &PlanHash, now_unix: u64, lifetime_secs: u64) -> Approval {
        let mut nonce = [0u8; 16];
        OsRng.fill_bytes(&mut nonce);
        let expires_unix = now_unix.saturating_add(lifetime_secs);
        let signature = self
            .0
            .sign(&message(plan_hash, expires_unix, &nonce))
            .to_bytes();
        Approval {
            expires_unix,
            nonce,
            signature,
        }
    }
}

/// Checks approvals and remembers used nonces until they expire.
pub struct Verifier {
    key: VerifyingKey,
    used: Mutex<HashMap<[u8; 16], u64>>,
}

impl Verifier {
    pub fn new(key: VerifyingKey) -> Self {
        Self {
            key,
            used: Mutex::new(HashMap::new()),
        }
    }

    /// Accepts `approval` for `plan_hash` at most once. The signature is
    /// checked first, so forged approvals can never use up a nonce.
    pub fn verify(
        &self,
        plan_hash: &PlanHash,
        approval: &Approval,
        now_unix: u64,
    ) -> Result<(), ApprovalError> {
        let msg = message(plan_hash, approval.expires_unix, &approval.nonce);
        self.key
            .verify_strict(&msg, &Signature::from_bytes(&approval.signature))
            .map_err(|_| ApprovalError::Auth)?;
        if now_unix > approval.expires_unix {
            return Err(ApprovalError::Expired);
        }
        if approval.expires_unix - now_unix > MAX_LIFETIME_SECS {
            return Err(ApprovalError::LifetimeTooLong);
        }
        let mut used = self.used.lock().unwrap_or_else(|e| e.into_inner());
        // Expired nonces can never verify again, so forgetting them is safe.
        used.retain(|_, expires| *expires >= now_unix);
        if used.insert(approval.nonce, approval.expires_unix).is_some() {
            return Err(ApprovalError::NonceReused);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;
    const PLAN: PlanHash = [7u8; 32];

    fn setup() -> (ApprovalKey, Verifier) {
        let key = ApprovalKey::generate();
        let verifier = Verifier::new(key.verifying_key());
        (key, verifier)
    }

    /// T-J2: valid, reused, tampered, expired.
    #[test]
    fn accepts_a_valid_approval_exactly_once() {
        let (key, verifier) = setup();
        let approval = key.sign(&PLAN, NOW, 300);
        assert_eq!(verifier.verify(&PLAN, &approval, NOW), Ok(()));
        assert_eq!(
            verifier.verify(&PLAN, &approval, NOW + 1),
            Err(ApprovalError::NonceReused)
        );
    }

    #[test]
    fn rejects_another_plan() {
        let (key, verifier) = setup();
        let approval = key.sign(&PLAN, NOW, 300);
        assert_eq!(
            verifier.verify(&[8u8; 32], &approval, NOW),
            Err(ApprovalError::Auth)
        );
    }

    #[test]
    fn rejects_an_extended_expiry_or_swapped_nonce() {
        let (key, verifier) = setup();
        let mut later = key.sign(&PLAN, NOW, 300);
        later.expires_unix += 3600;
        assert_eq!(
            verifier.verify(&PLAN, &later, NOW),
            Err(ApprovalError::Auth)
        );
        let mut swapped = key.sign(&PLAN, NOW, 300);
        swapped.nonce[0] ^= 1;
        assert_eq!(
            verifier.verify(&PLAN, &swapped, NOW),
            Err(ApprovalError::Auth)
        );
    }

    #[test]
    fn rejects_expired_and_overlong_approvals() {
        let (key, verifier) = setup();
        let approval = key.sign(&PLAN, NOW, 300);
        assert_eq!(
            verifier.verify(&PLAN, &approval, NOW + 301),
            Err(ApprovalError::Expired)
        );
        let overlong = key.sign(&PLAN, NOW, MAX_LIFETIME_SECS + 1);
        assert_eq!(
            verifier.verify(&PLAN, &overlong, NOW),
            Err(ApprovalError::LifetimeTooLong)
        );
    }

    #[test]
    fn rejects_another_key() {
        let (_, verifier) = setup();
        let approval = ApprovalKey::generate().sign(&PLAN, NOW, 300);
        assert_eq!(
            verifier.verify(&PLAN, &approval, NOW),
            Err(ApprovalError::Auth)
        );
    }

    #[test]
    fn forgets_nonces_after_they_expire() {
        let (key, verifier) = setup();
        for i in 0..100 {
            let approval = key.sign(&PLAN, NOW + i * 1000, 10);
            verifier.verify(&PLAN, &approval, NOW + i * 1000).unwrap();
        }
        assert_eq!(verifier.used.lock().unwrap().len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn key_is_sealed_on_disk_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys").join("approval.key.dpapi");
        let first = ApprovalKey::load_or_create(&path).unwrap();
        let sealed = fs::read(&path).unwrap();
        let secret = first.0.to_bytes();
        assert!(!sealed.windows(32).any(|w| w == secret));

        let again = ApprovalKey::load_or_create(&path).unwrap();
        assert_eq!(again.verifying_key(), first.verifying_key());

        // A damaged key file is set aside and replaced with a working key.
        let mut tampered = sealed.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        fs::write(&path, &tampered).unwrap();
        let replaced = ApprovalKey::load_or_create(&path).unwrap();
        assert_ne!(replaced.verifying_key(), first.verifying_key());
        assert_eq!(
            fs::read(path.with_extension("unreadable")).unwrap(),
            tampered
        );
        let reloaded = ApprovalKey::load_or_create(&path).unwrap();
        assert_eq!(reloaded.verifying_key(), replaced.verifying_key());

        // So is an empty one, as a crash during the first write could leave.
        fs::write(&path, b"").unwrap();
        assert!(ApprovalKey::load_or_create(&path).is_ok());
    }
}
