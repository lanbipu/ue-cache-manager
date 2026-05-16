//! `uecm-cli winrm <action>` handlers.

use crate::cli::args::WinrmAction;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::core::{bootstrap, winrm};
use crate::error::UecmResult;
use serde::Serialize;

#[derive(Serialize)]
struct ProbeOut {
    host: String,
    ok: bool,
    message: String,
    latency_ms: i64,
}

pub fn handle(ctx: &mut Ctx<'_>, action: WinrmAction) -> UecmResult<()> {
    match action {
        WinrmAction::Probe { host } => probe(ctx, &host),
        WinrmAction::BootstrapScript { output } => bootstrap_script(ctx, output),
        WinrmAction::Bootstrap { host, user, pass, enable_local_admin } => {
            bootstrap_remote(ctx, &host, &user, &pass, enable_local_admin)
        }
    }
}

fn probe(ctx: &mut Ctx<'_>, host: &str) -> UecmResult<()> {
    let result = winrm::probe(host)?;
    let out = ProbeOut {
        host: host.into(),
        ok: result.ok,
        message: result.message,
        latency_ms: result.latency_ms,
    };
    ctx.emitter.emit_result(&out).ok();
    Ok(())
}

fn bootstrap_script(ctx: &mut Ctx<'_>, output_path: Option<String>) -> UecmResult<()> {
    let body = bootstrap::manual_winrm_script();
    match output_path {
        Some(p) => {
            std::fs::write(&p, &body).map_err(|e| {
                crate::error::UecmError::Configuration(format!("write {}: {}", p, e))
            })?;
            let summary = serde_json::json!({ "written_to": p, "bytes": body.len() });
            ctx.emitter
                .emit_event(&crate::cli::output::Event::Completed { summary })
                .ok();
        }
        None => {
            // Print raw script to stdout (no JSON wrapping — caller redirects to .ps1)
            print!("{}", body);
        }
    }
    Ok(())
}

fn bootstrap_remote(
    ctx: &mut Ctx<'_>,
    host: &str,
    user: &str,
    pass: &str,
    enable_local_admin: bool,
) -> UecmResult<()> {
    let result = bootstrap::enable_winrm_with_psexec(host, user, pass, enable_local_admin)?;
    ctx.emitter.emit_result(&result).ok();
    Ok(())
}
