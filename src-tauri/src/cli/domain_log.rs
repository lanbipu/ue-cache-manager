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
            let creds = cred.resolve(db)?;
            let report = ue_log_verify::run_for_host(
                &host,
                &editor_exe,
                &project,
                timeout,
                creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
            )?;
            ctx.emitter.emit_result(&report).ok();
            Ok(())
        }
    }
}
