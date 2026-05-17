//! `uecm-cli share <action>` handlers — full impl arrives in Phase 2.

use crate::cli::args::ShareAction;
use crate::cli::run::Ctx;
use crate::error::{UecmError, UecmResult};

pub fn handle(_ctx: &mut Ctx<'_>, _action: ShareAction) -> UecmResult<()> {
    Err(UecmError::OperationFailed("share: not yet implemented".into()))
}
