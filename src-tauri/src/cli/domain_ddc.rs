//! `uecm-cli ddc <action>` handlers.
//!
//! generate — runs UE with -DDC=CreatePak on the source machine, streams
//!            UeRunnerEvents as NDJSON until the process terminates.
//! verify   — checks that a .ddp file exists and is non-zero on the source machine.
//! distribute — Robocopy fan-out from source to one or more target machines.

use crate::cli::args::DdcAction;
use crate::cli::credential_args::CredentialArgs;
use crate::cli::output::{EmitSerialize, Event};
use crate::cli::run::Ctx;
use crate::core::ddc_pak;
use crate::core::pak_distribute;
use crate::core::ue_runner::{UeRunnerBackend, UeRunnerEvent};
use crate::data::{machines as data_machines, project_locations};
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: DdcAction) -> UecmResult<()> {
    match action {
        DdcAction::Generate { project_id, source_machine, cred } => {
            generate(ctx, project_id, source_machine, &cred)
        }
        DdcAction::Verify { project_id, source_machine, cred } => {
            verify(ctx, project_id, source_machine, &cred)
        }
        DdcAction::Distribute { project_id, source_machine, targets, cred } => {
            distribute(ctx, project_id, source_machine, &targets, &cred)
        }
    }
}

// ─── generate ─────────────────────────────────────────────────────────────────

fn generate(
    ctx: &mut Ctx<'_>,
    project_id: i64,
    source_machine_id: i64,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;

    let machine = data_machines::find_by_id(db, source_machine_id)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("machine {} not found", source_machine_id))
    })?;
    let location =
        project_locations::get_for_project_machine(db, project_id, source_machine_id)?
            .ok_or_else(|| {
                UecmError::InvalidInput(format!(
                    "project {} not located on machine {}",
                    project_id, source_machine_id
                ))
            })?;

    let engine_path = resolve_engine_path(db, source_machine_id)?;

    let (op_user, op_pass) = resolve_creds(db, cred)?;

    // Pick backend: if the machine's IP resolves to loopback, run locally.
    let backend = if crate::core::loopback::is_loopback_target(&machine.ip)
        || crate::core::loopback::is_loopback_target(&machine.hostname)
    {
        UeRunnerBackend::Local
    } else {
        UeRunnerBackend::Remote
    };

    // Preflight only makes sense for remote (needs WinRM to check paths).
    if matches!(backend, UeRunnerBackend::Remote) {
        ddc_pak::preflight(
            &machine.ip,
            &engine_path,
            &location.uproject_path,
            op_user.as_deref(),
            op_pass.as_deref(),
        )?;
    }

    let mut handle = ddc_pak::launch_generation(
        backend,
        &machine.ip,
        &engine_path,
        &location.uproject_path,
        op_user.as_deref(),
        op_pass.as_deref(),
    );

    // ue_runner::run() spawns a tokio task internally, so we need a runtime to
    // drive it. The CLI binary doesn't have a global runtime — build one here.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| UecmError::OperationFailed(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        while let Some(ev) = handle.events.recv().await {
            match ev {
                UeRunnerEvent::Spawned { pid, log_path } => {
                    ctx.emitter
                        .emit_event(&Event::Spawned { pid, log_path })
                        .ok();
                }
                UeRunnerEvent::LogLine { text, parsed_kind } => {
                    ctx.emitter
                        .emit_event(&Event::LogLine { text, parsed_kind })
                        .ok();
                }
                UeRunnerEvent::Progress { pct, label } => {
                    ctx.emitter
                        .emit_event(&Event::Progress {
                            pct,
                            label,
                            current: None,
                            total: None,
                        })
                        .ok();
                }
                UeRunnerEvent::Completed { exit_code, log_tail } => {
                    ctx.emitter
                        .emit_event(&Event::Completed {
                            summary: serde_json::json!({
                                "exit_code": exit_code,
                                "log_tail": log_tail,
                            }),
                        })
                        .ok();
                    break;
                }
                UeRunnerEvent::Cancelled => {
                    ctx.emitter
                        .emit_event(&Event::Cancelled {
                            reason: "external".into(),
                        })
                        .ok();
                    break;
                }
                UeRunnerEvent::Error { message } => {
                    return Err(UecmError::OperationFailed(format!(
                        "ue runner: {}",
                        message
                    )));
                }
            }
        }
        Ok::<_, UecmError>(())
    })?;

    Ok(())
}

// ─── verify ───────────────────────────────────────────────────────────────────

fn verify(
    ctx: &mut Ctx<'_>,
    project_id: i64,
    source_machine_id: i64,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;

    let machine = data_machines::find_by_id(db, source_machine_id)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("machine {} not found", source_machine_id))
    })?;
    let location =
        project_locations::get_for_project_machine(db, project_id, source_machine_id)?
            .ok_or_else(|| {
                UecmError::InvalidInput(format!(
                    "project {} not located on machine {}",
                    project_id, source_machine_id
                ))
            })?;

    let (op_user, op_pass) = resolve_creds(db, cred)?;

    let output = ddc_pak::verify_output(
        &machine.ip,
        &location.abs_path,
        op_user.as_deref(),
        op_pass.as_deref(),
    )?;

    ctx.emitter.emit_result(&output).ok();
    Ok(())
}

// ─── distribute ───────────────────────────────────────────────────────────────

fn distribute(
    ctx: &mut Ctx<'_>,
    project_id: i64,
    source_machine_id: i64,
    target_ids: &[i64],
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;

    let source_machine = data_machines::find_by_id(db, source_machine_id)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("machine {} not found", source_machine_id))
    })?;
    let source_location =
        project_locations::get_for_project_machine(db, project_id, source_machine_id)?
            .ok_or_else(|| {
                UecmError::InvalidInput(format!(
                    "project {} not located on machine {}",
                    project_id, source_machine_id
                ))
            })?;

    let (op_user, op_pass) = resolve_creds(db, cred)?;

    let profile = pak_distribute::DistributeProfile::ddc_pak();
    let plan = pak_distribute::plan(
        &profile,
        db,
        source_machine_id,
        &source_machine.ip,
        &source_location,
        target_ids,
        project_id,
        None, // named_share_unc — not exposed in CLI for now
        op_user.clone(),
        op_pass.clone(),
        op_user,
        op_pass,
    )?;

    if plan.is_empty() {
        return Err(UecmError::InvalidInput(
            "distribution plan has no non-source targets".into(),
        ));
    }

    let total = plan.len() as i64;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| UecmError::OperationFailed(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        for (idx, item) in plan.into_iter().enumerate() {
            let item_id = format!("machine:{}", item.target_machine_id);

            ctx.emitter
                .emit_event(&Event::ItemStarted {
                    item_id: item_id.clone(),
                    index: idx as i64,
                    total,
                })
                .ok();

            let outcome = pak_distribute::run_one_with_profile(&profile, item).await;

            match outcome {
                Ok(out) => {
                    let msg = if out.ok {
                        None
                    } else {
                        Some(
                            out.message
                                .unwrap_or_else(|| out.stdout_tail.clone()),
                        )
                    };
                    ctx.emitter
                        .emit_event(&Event::ItemCompleted {
                            item_id,
                            index: idx as i64,
                            ok: out.ok,
                            message: msg,
                        })
                        .ok();
                    if !out.ok {
                        return Err(UecmError::OperationFailed(format!(
                            "robocopy exit {} on machine {}",
                            out.exit_code, out.target_machine_id
                        )));
                    }
                }
                Err(e) => {
                    ctx.emitter
                        .emit_event(&Event::ItemCompleted {
                            item_id,
                            index: idx as i64,
                            ok: false,
                            message: Some(e.to_string()),
                        })
                        .ok();
                    return Err(e);
                }
            }
        }
        Ok::<_, UecmError>(())
    })?;

    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({"distributed": true}),
        })
        .ok();

    Ok(())
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn resolve_creds(
    db: &crate::data::Db,
    cred: &CredentialArgs,
) -> UecmResult<(Option<String>, Option<String>)> {
    match cred.resolve(db)? {
        Some((u, p)) => Ok((Some(u), Some(p))),
        None => Ok((None, None)),
    }
}

fn resolve_engine_path(db: &crate::data::Db, machine_id: i64) -> UecmResult<String> {
    let installs = crate::data::machine_ue_installs::list_for_machine(db, machine_id)?;
    if installs.is_empty() {
        return Err(UecmError::InvalidInput(format!(
            "machine {} has no detected UE installs",
            machine_id
        )));
    }
    let install = installs
        .iter()
        .find(|i| i.is_primary)
        .cloned()
        .unwrap_or_else(|| installs[0].clone());
    Ok(install.install_path)
}
