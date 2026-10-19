//! Private authenticated protocol for the dedicated Excel worker. These types
//! describe data and fixed operations; they never constitute user authority.
use crate::app_adapter::CellEdit;
use custodian_winsec::excel::Snapshot;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub session: uuid::Uuid,
    pub source: PathBuf,
    pub output: PathBuf,
    pub existing_pids: Vec<u32>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub session: uuid::Uuid,
    pub sequence: u32,
    pub operation: Action,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Open,
    Metadata,
    Inspect,
    Edit {
        expected: Snapshot,
        sheet_id: String,
        cells: Vec<CellEdit>,
    },
    Save {
        expected: Snapshot,
    },
    Close {
        expected: Snapshot,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub session: uuid::Uuid,
    pub sequence: u32,
    pub result: Data,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Data {
    Started { pid: u32, birth: u64, window: i64 },
    Opened { window: i64 },
    Metadata { window: i64, saved: bool },
    Snapshot { snapshot: Snapshot },
    Saved,
    Closed,
    Refused,
}
