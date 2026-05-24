//! CLI implementation for `uecm-cli`. Bypasses Tauri runtime; calls core/data directly.

pub mod args;
pub mod config_file;
pub mod stdin_input;
pub mod credential_args;
pub mod destructive;
pub mod host_args;
pub mod output;
pub mod run;
pub mod domain_system;
pub mod domain_machine;
pub mod domain_winrm;
pub mod domain_ssh;
pub mod domain_cred;
pub mod domain_secret;
pub mod domain_env;
pub mod domain_ini;
pub mod domain_share;
pub mod domain_project;
pub mod domain_health;
pub mod domain_gpu;
pub mod domain_ddc;
pub mod domain_pso;
pub mod domain_log;
pub mod domain_local_cache;
pub mod domain_deploy;
pub mod domain_zen;
pub mod envelope;

// Re-export the emitter trait + the generic extension trait so domain handlers
// can `use crate::cli::{Emitter, EmitSerialize}` in one line.
pub use output::{Emitter, EmitSerialize};
