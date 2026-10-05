//! Approvals for clean-up plans (A-09.1, SR-D1). Deliberately **not** a
//! webview command: the approval key must never be reachable from a webview
//! (build plan step 84, T-KEYS-02).
//!
//! A person confirms with Windows Hello (or, where Hello is not set up, in a
//! native dialog that script in the webview cannot click). Only then is the
//! plan's digest signed with the approval key, which is sealed on disk with
//! DPAPI, and the quarantine checks that signature before it moves anything.

use custodian_approval::{Approval, ApprovalKey, PlanHash, Verifier};
use std::io;
use std::path::Path;

/// How long a signed approval stays valid. It is used straight away.
const APPROVAL_LIFETIME_SECS: u64 = 120;

pub struct ApprovalService {
    key: ApprovalKey,
    verifier: Verifier,
}

impl ApprovalService {
    /// Loads the DPAPI-sealed approval key at `key_path`, creating it on first run.
    pub fn load(key_path: &Path) -> io::Result<Self> {
        Ok(Self::with_key(ApprovalKey::load_or_create(key_path)?))
    }

    pub fn with_key(key: ApprovalKey) -> Self {
        let verifier = Verifier::new(key.verifying_key());
        Self { key, verifier }
    }

    pub fn verifier(&self) -> &Verifier {
        &self.verifier
    }

    /// Signs `plan_hash`. Call only after the person has said yes, and with
    /// the time they said it, so the approval's lifetime starts then.
    pub fn sign(&self, plan_hash: &PlanHash, now_unix: u64) -> Approval {
        self.key.sign(plan_hash, now_unix, APPROVAL_LIFETIME_SECS)
    }
}
