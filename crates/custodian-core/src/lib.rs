//! Custodian core: file scanning, duplicate detection, safe (undoable)
//! clean-up, process supervision, the IPC routing policy and the install-lab
//! helpers.

pub mod backup_check;
pub mod cleanup;
pub mod downloads;
pub mod duplicates;
pub mod executor;
pub mod hashing;
pub mod indexer;
pub mod install_lab;
pub mod install_lab_parsers;
pub mod install_lab_verify;
pub mod install_recipe;
pub mod install_report;
pub mod install_undo;
pub mod largest;
pub mod online_sm;
pub mod pipeline;
pub mod router;
pub mod safe_mode;
pub mod sandbox_factory;
pub mod supervisor;
pub mod uninstall_prep;
pub mod walk;
