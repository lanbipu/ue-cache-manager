//! `uecm-cli machine <action>` handlers. Full impl arrives in Task 3.x.

use crate::cli::args::MachineAction;
use crate::cli::run::Ctx;
use crate::error::{UecmError, UecmResult};

pub fn handle(_ctx: &mut Ctx<'_>, _action: MachineAction) -> UecmResult<()> {
    Err(UecmError::OperationFailed("machine: not yet implemented".into()))
}
