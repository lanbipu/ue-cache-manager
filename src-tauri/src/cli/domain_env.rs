//! `uecm-cli env <action>` handlers — full impl arrives in Phase 2.

use crate::cli::args::EnvAction;
use crate::cli::run::Ctx;
use crate::error::{UecmError, UecmResult};

pub fn handle(_ctx: &mut Ctx<'_>, _action: EnvAction) -> UecmResult<()> {
    Err(UecmError::OperationFailed("env: not yet implemented".into()))
}
