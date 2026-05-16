//! CLI implementation for `uecm-cli`. Bypasses Tauri runtime; calls core/data directly.

pub mod args;
pub mod credential_args;
pub mod host_args;
pub mod output;
pub mod run;
pub mod domain_system;
pub mod domain_machine;
pub mod domain_winrm;

// Re-export the emitter trait + the generic extension trait so domain handlers
// can `use crate::cli::{Emitter, EmitSerialize}` in one line.
pub use output::{Emitter, EmitSerialize};
