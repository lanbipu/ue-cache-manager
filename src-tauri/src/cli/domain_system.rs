//! `uecm-cli system <action>` handlers. Full impl arrives in Task 2.1+.

use crate::cli::args::SystemAction;
use crate::cli::run::Ctx;
use crate::error::{UecmError, UecmResult};

pub fn handle(_ctx: &mut Ctx<'_>, _action: SystemAction) -> UecmResult<()> {
    Err(UecmError::OperationFailed("system: not yet implemented".into()))
}
