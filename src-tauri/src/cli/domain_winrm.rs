//! `uecm-cli winrm <action>` handlers.

use crate::cli::args::WinrmAction;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::core::{bootstrap, winrm};
use crate::error::{UecmError, UecmResult};
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
        WinrmAction::Bootstrap { host, user, pass, pass_stdin, enable_local_admin } => {
            let password = read_bootstrap_password(pass.as_deref(), pass_stdin)?;
            bootstrap_remote(ctx, &host, &user, &password, enable_local_admin)
        }
    }
}

fn read_bootstrap_password(pass_inline: Option<&str>, pass_stdin: bool) -> UecmResult<String> {
    use std::io::{self, BufRead};
    if let Some(p) = pass_inline {
        return Ok(p.to_string());
    }
    if pass_stdin {
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line).map_err(|e| {
            UecmError::InvalidInput(format!("read password from stdin: {}", e))
        })?;
        return Ok(line.trim_end_matches(['\r', '\n']).to_string());
    }
    Err(UecmError::InvalidInput(
        "winrm bootstrap requires --pass or --pass-stdin".into(),
    ))
}

fn probe(ctx: &mut Ctx<'_>, host: &str) -> UecmResult<()> {
    let result = winrm::probe(host)?;
    if !result.ok {
        // Failure: don't emit a separate result payload first — the run()
        // dispatcher will emit a single `error` event with the script's
        // message embedded, which keeps `--json` consumers parsing exactly
        // one value per invocation.
        return Err(UecmError::PowerShell(format!(
            "winrm probe of {} reported failure: {}",
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
            if ctx.json_mode {
                // --json: wrap the script body in a structured payload so the
                // caller doesn't have to switch parsers.
                ctx.emitter
                    .emit_result(&serde_json::json!({ "script": body }))
                    .ok();
            } else {
                // Human: dump raw script to stdout so `... > setup.ps1` works.
                print!("{}", body);
            }
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
    let bootstrap_ok = result.ok;
    let message = result.message.clone();
    ctx.emitter.emit_result(&result).ok();
    if !bootstrap_ok {
        return Err(UecmError::OperationFailed(format!(
            "winrm bootstrap of {} reported failure: {}",
            host, message
        )));
    }
    Ok(())
}
