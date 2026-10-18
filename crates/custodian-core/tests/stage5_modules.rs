//! Compile the Stage 5 modules before their public `lib.rs` exports are wired.
//! The module files carry their own tempfile tests; this harness keeps that
//! verification available without changing the existing integration surface.

#[path = "../src/governed_recycle.rs"]
mod governed_recycle;
#[path = "../src/versioned_backup.rs"]
mod versioned_backup;
