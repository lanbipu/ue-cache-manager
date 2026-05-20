//! `uecm-cli winrm <action>` handlers.

use crate::cli::args::WinrmAction;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::core::{bootstrap, preflight, winrm};
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
        WinrmAction::Preflight { host, user, pass, pass_stdin, probe: with_probe } => {
            let password = read_bootstrap_password(pass.as_deref(), pass_stdin)?;
            preflight_path_b(ctx, &host, &user, &password, with_probe)
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
        "this command requires --pass or --pass-stdin".into(),
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
    // `winrm bootstrap` keeps its existing scope (WinRM core only); full render-node
    // provisioning is done via `machine authorize`.
    let result = bootstrap::enable_winrm_with_psexec(host, user, pass, enable_local_admin, false)?;
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

fn preflight_path_b(
    ctx: &mut Ctx<'_>,
    host: &str,
    user: &str,
    pass: &str,
    with_probe: bool,
) -> UecmResult<()> {
    let result = preflight::preflight_path_b(host, user, pass, with_probe)?;
    ctx.emitter.emit_result(&result).ok();
    // Exit code mapping:
    //   viable / likely_viable → 0 (Path B is usable)
    //   blocked                → non-zero (target rejects Path B)
    //   uncertain              → non-zero (operator-side config issue — most
    //                            commonly missing PsExec64; the verdict cannot
    //                            be trusted, so do NOT let automation proceed)
    if result.verdict == "blocked" || result.verdict == "uncertain" {
        return Err(UecmError::OperationFailed(format!(
            "Path B preflight of {} verdict={}: {}",
            host, result.verdict, result.reason
        )));
    }
    Ok(())
}
