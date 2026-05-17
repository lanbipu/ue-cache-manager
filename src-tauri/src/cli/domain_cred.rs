//! `uecm-cli cred <action>` handlers.

use crate::cli::args::CredAction;
use crate::cli::output::EmitSerialize;
use crate::cli::run::Ctx;
use crate::data::credentials as data_creds;
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: CredAction) -> UecmResult<()> {
    match action {
        CredAction::List => list(ctx),
        CredAction::Save { .. } => Err(UecmError::OperationFailed("save: pending Task 2.2".into())),
        CredAction::Delete { .. } => {
            Err(UecmError::OperationFailed("delete: pending Task 2.3".into()))
        }
    }
}

fn list(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = data_creds::list_all(db)?;
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}
