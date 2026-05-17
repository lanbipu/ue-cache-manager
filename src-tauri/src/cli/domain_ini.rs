//! `uecm-cli ini <action>` handlers — full impl arrives in Phase 2.

use crate::cli::args::IniAction;
use crate::cli::run::Ctx;
use crate::error::{UecmError, UecmResult};

pub fn handle(_ctx: &mut Ctx<'_>, _action: IniAction) -> UecmResult<()> {
    Err(UecmError::OperationFailed("ini: not yet implemented".into()))
}
