// Example code that deserializes and serializes the model.
// extern crate serde;
// #[macro_use]
// extern crate serde_derive;
// extern crate serde_json;
//
// use generated_module::lib;
//
// fn main() {
//     let json = r#"{"answer": 42}"#;
//     let model: lib = serde_json::from_str(&json).unwrap();
// }

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lib {
    pub approvals: Option<Vec<SignedApproval>>,

    pub hash_after: Option<String>,

    pub hash_before: Option<String>,

    pub id: String,

    pub kind: Kind,

    pub paths: Vec<String>,

    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedApproval {
    pub id: String,

    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Create,

    Delete,

    Modify,
}
