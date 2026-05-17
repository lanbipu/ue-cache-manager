//! `uecm-cli gpu <action>` handlers.

use crate::cli::args::GpuAction;
use crate::cli::output::EmitSerialize;
use crate::cli::run::Ctx;
use crate::error::UecmResult;

pub fn handle(ctx: &mut Ctx<'_>, action: GpuAction) -> UecmResult<()> {
    match action {
        GpuAction::Matrix => matrix(ctx),
    }
}

fn matrix(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let m = crate::core::gpu_consistency::build_matrix(db)?;
    ctx.emitter.emit_result(&m).ok();
    Ok(())
}
