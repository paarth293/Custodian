//! Wire types shared across processes, mirroring `proto/core/Transaction.json`.
//! Regenerate with `npm run proto:gen`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approvals: Option<Vec<SignedApproval>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_before: Option<String>,
    pub id: String,
    pub kind: Kind,
    pub paths: Vec<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignedApproval {
    pub id: String,
    pub signature: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Create,
    Delete,
    Modify,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_minimal_transaction() {
        let tx: Transaction = serde_json::from_str(
            r#"{"id":"1","kind":"modify","paths":["a"],"timestamp":"2026-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        assert_eq!(tx.kind, Kind::Modify);
        assert!(tx.approvals.is_none());
    }
}
