//! `uecm-cli ssh <action>` handlers — SSH transport onboarding + probe.
//! Replaces the retiring `winrm` command domain (P5a adds package-bootstrap +
//! authorize; P1 ships `probe`).

use crate::cli::args::SshAction;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::core::ssh::{RemoteExecutor, SshExecutor};
use crate::error::{UecmError, UecmResult};

/// Top-level result object — mirrors `winrm probe`'s `ProbeOut` shape so CLI /
/// JSON automation that parsed `{host, ok, message, latency_ms}` from
/// `winrm probe` keeps working against `ssh probe`.
#[derive(serde::Serialize)]
struct ProbeOut {
    host: String,
    ok: bool,
    message: String,
    latency_ms: i64,
}

pub fn handle(ctx: &mut Ctx<'_>, action: SshAction) -> UecmResult<()> {
    match action {
        SshAction::Probe { host } => probe(ctx, &host),
    }
}

fn probe(ctx: &mut Ctx<'_>, host: &str) -> UecmResult<()> {
    let exec = SshExecutor::from_config()?;
    let result = exec.probe(host, None)?;
    if !result.ok {
        // Failure: let run()'s dispatcher emit a single `error` event (one value
        // per invocation), same as winrm probe.
        return Err(UecmError::SshConnect(format!(
            "ssh probe of {} reported failure: {}",
            host, result.message
        )));
    }
    let out = ProbeOut {
        host: host.into(),
        ok: result.ok,
        message: result.message,
        latency_ms: result.latency_ms,
    };
    ctx.emitter.emit_result(&out).ok();
    Ok(())
}
