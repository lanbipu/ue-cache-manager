//! `uecm-cli log <action>` handlers.
use crate::cli::args::LogAction;
use crate::cli::output::EmitSerialize;
use crate::cli::run::Ctx;
use crate::core::ue_log_verify;
use crate::error::UecmResult;

pub fn handle(ctx: &mut Ctx<'_>, action: LogAction) -> UecmResult<()> {
    match action {
        LogAction::VerifyStartup { host, editor_exe, project, timeout, cred } => {
            let db = ctx.require_db()?;
            cred.preflight(db)?;
            let report = ue_log_verify::run_for_host(
                &host,
                &editor_exe,
                &project,
                timeout,
                None,
            )?;
            ctx.emitter.emit_result(&report).ok();
            Ok(())
        }
    }
}
