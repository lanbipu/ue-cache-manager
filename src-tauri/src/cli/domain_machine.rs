//! `uecm-cli machine <action>` handlers.

use crate::cli::args::MachineAction;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::data::machines;
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: MachineAction) -> UecmResult<()> {
    match action {
        MachineAction::List => list(ctx),
        MachineAction::Scan { .. } => Err(UecmError::OperationFailed("scan: pending Task 3.3".into())),
        MachineAction::Add { .. } => Err(UecmError::OperationFailed("add: pending Task 3.2".into())),
        MachineAction::Refresh { .. } => {
            Err(UecmError::OperationFailed("refresh: pending Task 3.4".into()))
        }
        MachineAction::Detail { .. } => {
            Err(UecmError::OperationFailed("detail: pending Task 3.2".into()))
        }
        MachineAction::Delete { .. } => {
            Err(UecmError::OperationFailed("delete: pending Task 3.2".into()))
        }
        MachineAction::Rename { .. } => {
            Err(UecmError::OperationFailed("rename: pending Task 3.2".into()))
        }
    }
}

fn list(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = machines::list_all(db)?;
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}
