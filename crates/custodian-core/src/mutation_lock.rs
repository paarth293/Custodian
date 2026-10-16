use std::sync::Mutex;

/// Serializes desktop file mutation, quarantine, purge and undo within this process.
pub(crate) static MUTATIONS: Mutex<()> = Mutex::new(());
