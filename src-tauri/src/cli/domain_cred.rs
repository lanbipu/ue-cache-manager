//! `uecm-cli cred <action>` handlers — full impl arrives in Phase 2.

use crate::cli::args::CredAction;
use crate::cli::run::Ctx;
use crate::error::{UecmError, UecmResult};

pub fn handle(_ctx: &mut Ctx<'_>, _action: CredAction) -> UecmResult<()> {
    Err(UecmError::OperationFailed("cred: not yet implemented".into()))
}
