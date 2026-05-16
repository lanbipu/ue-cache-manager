//! `uecm-cli winrm <action>` handlers. Full impl arrives in Task 3.x.

use crate::cli::args::WinrmAction;
use crate::cli::run::Ctx;
use crate::error::{UecmError, UecmResult};

pub fn handle(_ctx: &mut Ctx<'_>, _action: WinrmAction) -> UecmResult<()> {
    Err(UecmError::OperationFailed("winrm: not yet implemented".into()))
}
