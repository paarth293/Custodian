//! Custodian core: file scanning, duplicate detection, safe (undoable)
//! clean-up, process supervision, the IPC routing policy and the install-lab
//! helpers.

pub mod backup_check;
pub mod cleanup;
pub mod content;
pub mod coordinator;
#[cfg(all(test, windows))]
mod crash_probe;
pub mod downloads;
pub mod duplicates;
pub mod executor;
pub mod file_tasks;
pub mod find;
pub mod hashing;
pub mod indexer;
pub mod install_lab;
pub mod install_lab_parsers;
pub mod install_lab_verify;
pub mod install_recipe;
pub mod install_report;
pub mod install_undo;
pub mod instance;
pub mod intent_plan;
pub mod largest;
mod mutation_lock;
pub mod online_sm;
pub mod pipeline;
pub mod privacy;
pub mod request;
pub mod router;
pub mod routine;
pub mod safe_mode;
pub mod sandbox_factory;
pub mod supervisor;
pub mod task_program;
pub mod temp;
pub mod uninstall_prep;
pub mod walk;
pub mod workflow;
