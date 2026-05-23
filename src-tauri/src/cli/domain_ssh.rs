//! `uecm-cli ssh <action>` handlers — SSH transport onboarding + probe.
//! Replaces the retiring `winrm` command domain (P5a adds package-bootstrap +
//! authorize; P1 ships `probe`).

use crate::cli::args::SshAction;
use crate::cli::output::Event;
use crate::cli::run::Ctx;
use crate::core::ssh::{RemoteExecutor, SshExecutor};
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: SshAction) -> UecmResult<()> {
    match action {
        SshAction::Probe { host } => probe(ctx, &host),
    }
}

fn probe(ctx: &mut Ctx<'_>, host: &str) -> UecmResult<()> {
    let exec = SshExecutor::from_config()?;
    let result = exec.probe(host, None)?;
    if !result.ok {
        return Err(UecmError::SshConnect(format!(
            "ssh probe of {} reported failure: {}",
            host, result.message
        )));
    }
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "host": host,
                "ok": result.ok,
                "message": result.message,
                "latency_ms": result.latency_ms,
            }),
        })
        .ok();
    Ok(())
}
