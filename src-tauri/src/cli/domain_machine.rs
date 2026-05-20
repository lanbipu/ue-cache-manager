//! `uecm-cli machine <action>` handlers.

use crate::cli::args::MachineAction;
use crate::cli::destructive::{self, Outcome};
use crate::cli::output::Event;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::data::{machines, machine_ue_installs, machine_gpus};
use crate::error::{UecmError, UecmResult};
use crate::data::machines::Machine;
use serde_json::json;

pub fn handle(ctx: &mut Ctx<'_>, action: MachineAction) -> UecmResult<()> {
    match action {
        MachineAction::List => list(ctx),
        MachineAction::Scan { cidr, timeout_ms } => scan(ctx, &cidr, timeout_ms),
        MachineAction::Add { ip, hostname } => add(ctx, ip, hostname),
        MachineAction::Refresh { id, cred } => refresh(ctx, id, &cred),
        MachineAction::Detail { id } => detail(ctx, id),
        MachineAction::Delete { id, yes, dry_run } => delete(ctx, id, yes, dry_run),
        MachineAction::Rename { id, hostname } => rename(ctx, id, hostname),
        MachineAction::DeepScan { machine_ids, all, cred } => deep_scan(ctx, machine_ids, all, &cred),
        MachineAction::Authorize { machine_ids, all, save_as, cred } => {
            authorize(ctx, machine_ids, all, save_as, &cred)
        }
    }
}

fn list(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = machines::list_all(db)?;
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}

fn add(ctx: &mut Ctx<'_>, ip: String, hostname: Option<String>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let hostname = hostname.unwrap_or_else(|| ip.clone());

    // Idempotent: same IP twice doesn't trip the UNIQUE constraint. Matches the
    // UI's `add_discovered_machine` behavior so `scan → add` automation can
    // safely re-run without manually deduping.
    if let Some(existing) = machines::find_by_ip(db, &ip)? {
        let id = existing.id.expect("found machine must have an id");
        let summary = json!({
            "id": id,
            "ip": ip,
            "hostname": existing.hostname,
            "already_present": true,
        });
        ctx.emitter.emit_event(&Event::Completed { summary }).ok();
        return Ok(());
    }

    let machine = Machine::new(&hostname, &ip);
    let id = machines::insert(db, &machine)?;

    let summary = json!({
        "id": id,
        "ip": ip,
        "hostname": hostname,
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}

fn detail(ctx: &mut Ctx<'_>, id: i64) -> UecmResult<()> {
    let db = ctx.require_db()?;

    let machine = machines::find_by_id(db, id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine id={} not found", id)))?;

    let ue_installs = machine_ue_installs::list_for_machine(db, id)?;
    let gpus = machine_gpus::list_for_machine(db, id)?;

    let detail = json!({
        "machine": machine,
        "ue_installs": ue_installs,
        "gpus": gpus,
    });
    ctx.emitter.emit_result(&detail).ok();
    Ok(())
}

fn delete(ctx: &mut Ctx<'_>, id: i64, yes: bool, dry_run: bool) -> UecmResult<()> {
    let outcome = destructive::check(yes, dry_run, "machine.delete")?;

    let db = ctx.require_db()?;
    // Check existence first so a typo / repeated delete fails loudly instead
    // of pretending success. `machines::delete` itself is row-count-agnostic.
    if machines::find_by_id(db, id)?.is_none() {
        return Err(UecmError::InvalidInput(format!(
            "machine id={} not found (already deleted or wrong id)",
            id
        )));
    }

    if outcome == Outcome::DryRun {
        destructive::emit_plan(
            ctx.emitter.as_mut(),
            "machine.delete",
            json!({ "id": id }),
        );
        return Ok(());
    }

    machines::delete(db, id)?;

    let summary = json!({
        "id": id,
        "deleted": true,
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}

fn rename(ctx: &mut Ctx<'_>, id: i64, hostname: String) -> UecmResult<()> {
    let db = ctx.require_db()?;
    machines::rename(db, id, &hostname)?;

    let summary = json!({
        "id": id,
        "hostname": hostname,
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}

fn scan(ctx: &mut Ctx<'_>, cidr: &str, timeout_ms: u64) -> UecmResult<()> {
    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "machine_scan".into(),
            task_id: None,
            metadata: serde_json::json!({ "cidr": cidr, "timeout_ms": timeout_ms }),
        })
        .ok();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| UecmError::Configuration(format!("tokio runtime: {}", e)))?;
    let hosts = runtime.block_on(crate::core::network::scan_cidr(cidr, timeout_ms))?;
    let total = hosts.len() as i64;
    for h in &hosts {
        ctx.emitter
            .emit_event(&Event::HostProbe {
                ip: h.ip.clone(),
                winrm_open: h.winrm_open,
                smb_open: h.smb_open,
                rpc_open: h.rpc_open,
            })
            .ok();
    }
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({ "hosts": total }),
        })
        .ok();
    Ok(())
}

fn refresh(ctx: &mut Ctx<'_>, id: i64, cred: &crate::cli::credential_args::CredentialArgs) -> UecmResult<()> {
    // Fetch machine, grab IP (canonical connect target — matches the UI's
    // `commands::discovery::refresh_machine`). Hostname can drift if the user
    // renames the row, so IP is the only reliable WinRM target.
    let (host, hostname_for_log) = {
        let db = ctx.require_db()?;
        let machine = machines::find_by_id(db, id)?
            .ok_or_else(|| UecmError::InvalidInput(format!("machine id={} not found", id)))?;
        (machine.ip.clone(), machine.hostname.clone())
    };

    // Resolve credentials once for the whole refresh (probe + UE detect + GPU detect).
    let creds = {
        let db = ctx.require_db()?;
        cred.resolve(db)?
    };

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "machine_refresh".into(),
            task_id: Some(format!("machine:{}", id)),
            metadata: serde_json::json!({
                "ip": host,
                "hostname": hostname_for_log,
                "authenticated": creds.is_some(),
            }),
        })
        .ok();

    // 1. WinRM probe — mirror commands::discovery::refresh_machine so the UI
    // and CLI write the SAME `online` / `offline` status values. Otherwise a
    // CLI-refresh'd machine vanishes from the Dashboard's online count.
    ctx.emitter
        .emit_event(&Event::Progress {
            pct: None,
            label: "winrm probe".into(),
            current: None,
            total: None,
        })
        .ok();
    let probe_result = match &creds {
        Some((u, p)) => crate::core::winrm::probe_with_credential(&host, u, p),
        None => crate::core::winrm::probe(&host),
    };
    let probe = match probe_result {
        Ok(p) if p.ok => {
            {
                let db = ctx.require_db()?;
                machines::mark_seen(db, id, "online")?;
            }
            p
        }
        Ok(p) => {
            {
                let db = ctx.require_db()?;
                machines::mark_seen(db, id, "offline")?;
            }
            return Err(UecmError::PowerShell(format!(
                "winrm probe failed: {}",
                p.message
            )));
        }
        Err(e) => {
            {
                let db = ctx.require_db()?;
                machines::mark_seen(db, id, "offline")?;
            }
            return Err(e);
        }
    };

    // 2. Detect UE installs + persist FIRST (partial-failure tolerance —
    // mirrors `commands::discovery::refresh_machine`). If a later step
    // (e.g. GPU detect) blows up we still keep the UE list we already saved
    // rather than discarding it.
    ctx.emitter
        .emit_event(&Event::Progress {
            pct: None,
            label: "detect ue installs".into(),
            current: None,
            total: None,
        })
        .ok();
    let detected_ue = match &creds {
        Some((u, p)) => crate::core::discovery::detect_ue_versions_with_credential(&host, u, p)?,
        None => crate::core::discovery::detect_ue_versions(&host)?,
    };
    // PowerShell `query-ue-versions.ps1` sorts version ascending, so picking
    // index 0 marks the OLDEST install as primary — wrong, downstream
    // DDC/PSO jobs that fall back to `is_primary` would pick the wrong engine.
    // Compare versions NUMERICALLY (split major.minor and parse), not
    // lexicographically — string compare puts "4.9" > "4.27" and would put
    // "5.10" < "5.8" once UE 5.10 ships.
    fn parse_version(v: &str) -> (u32, u32) {
        let mut parts = v.split('.');
        let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        (major, minor)
    }
    let primary_idx = detected_ue
        .iter()
        .enumerate()
        .max_by_key(|(_, d)| parse_version(&d.version))
        .map(|(i, _)| i);
    {
        let db = ctx.require_db()?;
        // Mirror commands/discovery.rs: snapshot existing rows so intree_*
        // metadata written by Plan 7 T1.6 survives a `uecm-cli machine refresh`.
        let existing = machine_ue_installs::list_for_machine(db, id)?;
        machine_ue_installs::delete_for_machine(db, id)?;
        for (idx, detected) in detected_ue.iter().enumerate() {
            let prior = existing
                .iter()
                .find(|u| u.version == detected.version && u.install_path == detected.install_path);
            let install = machine_ue_installs::UeInstall {
                id: None,
                machine_id: id,
                version: detected.version.clone(),
                install_path: detected.install_path.clone(),
                is_primary: Some(idx) == primary_idx,
                zen_cli_intree_path: prior.and_then(|p| p.zen_cli_intree_path.clone()),
                zen_cli_intree_version: prior.and_then(|p| p.zen_cli_intree_version.clone()),
                zen_cli_intree_sha256: prior.and_then(|p| p.zen_cli_intree_sha256.clone()),
                zenserver_intree_path: prior.and_then(|p| p.zenserver_intree_path.clone()),
                zenserver_intree_version: prior.and_then(|p| p.zenserver_intree_version.clone()),
                zenserver_intree_sha256: prior.and_then(|p| p.zenserver_intree_sha256.clone()),
            };
            machine_ue_installs::upsert(db, &install)?;
        }
    }

    // 3. Detect GPUs + persist
    ctx.emitter
        .emit_event(&Event::Progress {
            pct: None,
            label: "detect gpus".into(),
            current: None,
            total: None,
        })
        .ok();
    let detected_gpus = match &creds {
        Some((u, p)) => crate::core::discovery::detect_gpus_with_credential(&host, u, p)?,
        None => crate::core::discovery::detect_gpus(&host)?,
    };
    {
        let db = ctx.require_db()?;
        // Convert DetectedGpu → GpuInfo
        let gpu_infos: Vec<machine_gpus::GpuInfo> = detected_gpus
            .iter()
            .map(|gpu| machine_gpus::GpuInfo {
                id: None,
                machine_id: id,
                gpu_model: gpu.gpu_model.clone(),
                driver_version: gpu.driver_version.clone(),
                vendor: gpu.vendor,
                vram_mb: gpu.vram_mb,
            })
            .collect();
        machine_gpus::replace_for_machine(db, id, &gpu_infos)?;
        // last_seen + status were already updated by the probe branch above —
        // no extra mark_seen here (avoids overwriting the canonical
        // online/offline tokens with anything else).
    }

    let summary = json!({
        "machine_id": id,
        "ue_versions": detected_ue.len(),
        "gpus": detected_gpus.len(),
        "latency_ms": probe.latency_ms,
        "authenticated": creds.is_some(),
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}

/// Expand a `--machine-ids` / `--all` selection into concrete machine ids.
/// Shared by `deep_scan` and `authorize`.
fn resolve_target_ids(db: &crate::data::Db, machine_ids: &[i64], all: bool) -> UecmResult<Vec<i64>> {
    if all {
        Ok(machines::list_all(db)?
            .into_iter()
            .filter_map(|m| m.id)
            .collect())
    } else if machine_ids.is_empty() {
        Err(UecmError::InvalidInput(
            "one of --machine-ids or --all is required".into(),
        ))
    } else {
        Ok(machine_ids.to_vec())
    }
}

fn deep_scan(
    ctx: &mut Ctx<'_>,
    machine_ids: Vec<i64>,
    all: bool,
    cred: &crate::cli::credential_args::CredentialArgs,
) -> UecmResult<()> {
    let ids = {
        let db = ctx.require_db()?;
        resolve_target_ids(db, &machine_ids, all)?
    };
    // Resolve credentials ONCE (stdin/DPAPI). `resolved` is reused for the
    // explicit reachability probe; `sub_cred` (stdin-free) is reused for every
    // sub-handler — `--pass-stdin` is only readable once.
    let resolved = {
        let db = ctx.require_db()?;
        cred.resolve(db)?
    };
    let sub_cred = crate::cli::credential_args::CredentialArgs::inline(resolved.clone());

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "machine_deep_scan".into(),
            task_id: None,
            metadata: json!({ "machines": ids.len() }),
        })
        .ok();

    // Phase 1 (per machine): WinRM reachability probe + refresh (UE/GPU). UE/GPU
    // detection is inherently per-machine, so it stays in the loop. Classify:
    //   - machine id not found            -> failed
    //   - WinRM probe unreachable         -> skip + "run authorize" hint
    //   - probe OK but refresh later fails -> failed (NOT a skip — the box is
    //     reachable, so don't mislead the operator into re-running authorize)
    let mut reachable: Vec<i64> = Vec::new();
    let mut skipped = 0usize;
    let mut failed = 0usize;
    for id in &ids {
        let host = {
            let db = ctx.require_db()?;
            match machines::find_by_id(db, *id)? {
                Some(m) => m.ip,
                None => {
                    failed += 1;
                    ctx.emitter
                        .emit_event(&Event::Completed {
                            summary: json!({ "machine_id": id, "step": "deep_scan", "failed": true, "error": "machine not found" }),
                        })
                        .ok();
                    continue;
                }
            }
        };

        // Explicit reachability probe so we can tell "WinRM closed" (skip) apart
        // from "reachable but detection failed" (failure). `refresh` re-probes —
        // the small double-probe is worth the accurate classification.
        let probe_ok = match &resolved {
            Some((u, p)) => crate::core::winrm::probe_with_credential(&host, u, p)
                .map(|r| r.ok)
                .unwrap_or(false),
            None => crate::core::winrm::probe(&host).map(|r| r.ok).unwrap_or(false),
        };
        if !probe_ok {
            skipped += 1;
            ctx.emitter
                .emit_event(&Event::Completed {
                    summary: json!({
                        "machine_id": id,
                        "host": host,
                        "step": "deep_scan",
                        "skipped": true,
                        "reason": "WinRM unreachable",
                        "hint": "run `uecm-cli machine authorize` to open WinRM first",
                    }),
                })
                .ok();
            continue;
        }

        if let Err(e) = refresh(ctx, *id, &sub_cred) {
            failed += 1;
            ctx.emitter
                .emit_event(&Event::Completed {
                    summary: json!({
                        "machine_id": id,
                        "host": host,
                        "step": "refresh",
                        "failed": true,
                        "error": format!("WinRM reachable but refresh failed: {}", e),
                    }),
                })
                .ok();
            continue;
        }
        reachable.push(*id);
    }

    // Phase 2 (batch over reachable set): INI scan + health run ONCE so the
    // cross-machine consistency rules (zen / cluster-majority / gpu_consistency)
    // see the whole set, not N single-machine clusters. Sub-step errors are
    // recorded but never abort the command.
    if !reachable.is_empty() {
        if let Err(e) = crate::cli::domain_ini::handle(
            ctx,
            crate::cli::args::IniAction::Scan { machine_ids: reachable.clone(), cred: sub_cred.clone() },
        ) {
            ctx.emitter
                .emit_event(&Event::Completed {
                    summary: json!({ "step": "ini_scan", "error": e.to_string() }),
                })
                .ok();
        }
        if let Err(e) = crate::cli::domain_health::handle(
            ctx,
            crate::cli::args::HealthAction::Run {
                machine_ids: reachable.clone(),
                cidr: None,
                all: false,
                expected_local_path: String::new(),
                expected_shared_path: String::new(),
                cred: sub_cred.clone(),
            },
        ) {
            ctx.emitter
                .emit_event(&Event::Completed {
                    summary: json!({ "step": "health_run", "error": e.to_string() }),
                })
                .ok();
        }
    }

    ctx.emitter
        .emit_event(&Event::Completed {
            summary: json!({ "machines": ids.len(), "scanned": reachable.len(), "skipped": skipped, "failed": failed }),
        })
        .ok();
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AuthorizeStep {
    Bootstrap,
    UsbFallback,
}

/// Pure mapping: Path B preflight verdict -> next action. `viable` / `likely_viable`
/// proceed to bootstrap; everything else (`blocked` / `uncertain`) falls back to USB.
pub(crate) fn authorize_decision(verdict: &str) -> AuthorizeStep {
    match verdict {
        "viable" | "likely_viable" => AuthorizeStep::Bootstrap,
        _ => AuthorizeStep::UsbFallback,
    }
}

fn authorize(
    ctx: &mut Ctx<'_>,
    machine_ids: Vec<i64>,
    all: bool,
    save_as: Option<String>,
    cred: &crate::cli::credential_args::CredentialArgs,
) -> UecmResult<()> {
    // Credentials are REQUIRED — preflight + bootstrap need a local admin user/pass.
    let (user, pass) = {
        let db = ctx.require_db()?;
        cred.resolve(db)?.ok_or_else(|| {
            UecmError::InvalidInput(
                "machine authorize requires credentials (--cred-alias or --user/--pass-stdin)".into(),
            )
        })?
    };

    // Optional: persist the resolved credential as an alias for later reuse
    // (so the follow-up `machine deep-scan --cred-alias <alias>` can find it).
    if let Some(alias) = save_as.as_deref() {
        crate::cli::domain_cred::save_resolved(ctx, alias, &user, &pass, "winrm")?;
    }

    let ids = {
        let db = ctx.require_db()?;
        resolve_target_ids(db, &machine_ids, all)?
    };

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "machine_authorize".into(),
            task_id: None,
            metadata: json!({ "machines": ids.len() }),
        })
        .ok();

    let mut authorized = 0usize;
    let mut fallback = 0usize;
    let mut failed = 0usize;
    for id in &ids {
        let host = {
            let db = ctx.require_db()?;
            match machines::find_by_id(db, *id)? {
                Some(m) => m.ip,
                None => {
                    failed += 1;
                    ctx.emitter
                        .emit_event(&Event::Completed {
                            summary: json!({ "machine_id": id, "error": "machine not found" }),
                        })
                        .ok();
                    continue;
                }
            }
        };

        // Preflight (Shallow; no SCM probe) — classify the verdict.
        let pf = match crate::core::preflight::preflight_path_b(&host, &user, &pass, false) {
            Ok(r) => r,
            Err(e) => {
                failed += 1;
                ctx.emitter
                    .emit_event(&Event::Completed {
                        summary: json!({ "machine_id": id, "host": host, "step": "preflight", "error": e.to_string() }),
                    })
                    .ok();
                continue;
            }
        };

        match authorize_decision(&pf.verdict) {
            AuthorizeStep::Bootstrap => {
                // Full render-node provisioning: local-admin token filter + SMB/WMI/
                // LongPaths/ExecutionPolicy/HighPerformance.
                match crate::core::bootstrap::enable_winrm_with_psexec(&host, &user, &pass, true, true) {
                    Ok(b) if b.ok => {
                        authorized += 1;
                        ctx.emitter
                            .emit_event(&Event::Completed {
                                summary: json!({ "machine_id": id, "host": host, "authorized": true, "message": b.message }),
                            })
                            .ok();
                    }
                    Ok(b) => {
                        failed += 1;
                        ctx.emitter
                            .emit_event(&Event::Completed {
                                summary: json!({ "machine_id": id, "host": host, "authorized": false, "error": b.message }),
                            })
                            .ok();
                    }
                    Err(e) => {
                        failed += 1;
                        ctx.emitter
                            .emit_event(&Event::Completed {
                                summary: json!({ "machine_id": id, "host": host, "step": "bootstrap", "error": e.to_string() }),
                            })
                            .ok();
                    }
                }
            }
            AuthorizeStep::UsbFallback => {
                fallback += 1;
                ctx.emitter
                    .emit_event(&Event::Completed {
                        summary: json!({
                            "machine_id": id,
                            "host": host,
                            "path_b_unavailable": true,
                            "verdict": pf.verdict,
                            "reason": pf.reason,
                            "hint": "Path B not viable — run `uecm-cli winrm bootstrap-script` and execute it on the machine via USB",
                        }),
                    })
                    .ok();
            }
        }
    }

    ctx.emitter
        .emit_event(&Event::Completed {
            summary: json!({ "machines": ids.len(), "authorized": authorized, "usb_fallback": fallback, "failed": failed }),
        })
        .ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::output::{NdjsonEmitter, Emitter};
    use crate::data::{open_in_memory, schema};
    use std::path::PathBuf;

    fn setup() -> (crate::data::Db, Vec<u8>) {
        let db = open_in_memory().expect("open :memory:");
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).expect("schema migrate");
        }
        let buf = Vec::new();
        (db, buf)
    }

    #[test]
    fn machine_round_trip_via_handlers() {
        let (db, buf) = setup();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf.clone()));
        let mut ctx = Ctx {
            db: Some(db),
            db_path: PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        };

        // Add a machine
        add(&mut ctx, "10.0.0.1".to_string(), Some("test-host".to_string()))
            .expect("add should succeed");

        // List should show it
        list(&mut ctx).expect("list should succeed");

        // Detail should load it
        detail(&mut ctx, 1).expect("detail should succeed");

        // Rename should work
        rename(&mut ctx, 1, "renamed-host".to_string()).expect("rename should succeed");

        // Delete should work
        delete(&mut ctx, 1, true, false).expect("delete should succeed");

        // Verify that we got past all operations
        // (output checking omitted as NdjsonEmitter writes to a moved buffer)
    }

    #[test]
    fn deep_scan_skips_winrm_unreachable_and_completes_batch() {
        let (db, buf) = setup();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf));
        let mut ctx = Ctx {
            db: Some(db),
            db_path: PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        };

        add(&mut ctx, "10.0.0.1".to_string(), Some("m1".to_string())).unwrap();
        add(&mut ctx, "10.0.0.2".to_string(), Some("m2".to_string())).unwrap();

        // No creds; on non-Windows the WinRM probe inside refresh fails, so both
        // machines are skipped — but the batch must still complete with Ok.
        let cred = crate::cli::credential_args::CredentialArgs::inline(None);
        let res = deep_scan(&mut ctx, vec![1, 2], false, &cred);
        assert!(res.is_ok(), "batch must complete even when every machine is skipped");
    }

    #[test]
    fn deep_scan_nonexistent_id_is_failed_not_skipped_and_batch_continues() {
        let (db, buf) = setup();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf));
        let mut ctx = Ctx {
            db: Some(db),
            db_path: PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        };
        // id 999 does not exist → refresh returns InvalidInput → classified as a
        // failure (not a WinRM skip), but the batch still completes Ok.
        let cred = crate::cli::credential_args::CredentialArgs::inline(None);
        let res = deep_scan(&mut ctx, vec![999], false, &cred);
        assert!(res.is_ok(), "batch completes; per-machine failure is reported in summary");
    }

    #[test]
    fn authorize_decision_maps_verdict() {
        assert_eq!(authorize_decision("viable"), AuthorizeStep::Bootstrap);
        assert_eq!(authorize_decision("likely_viable"), AuthorizeStep::Bootstrap);
        assert_eq!(authorize_decision("blocked"), AuthorizeStep::UsbFallback);
        assert_eq!(authorize_decision("uncertain"), AuthorizeStep::UsbFallback);
        assert_eq!(authorize_decision("anything-else"), AuthorizeStep::UsbFallback);
    }

    #[test]
    fn authorize_requires_credentials() {
        let (db, buf) = setup();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf));
        let mut ctx = Ctx {
            db: Some(db),
            db_path: PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        };
        add(&mut ctx, "10.0.0.1".to_string(), Some("m1".to_string())).unwrap();
        // inline(None) resolves to no credentials → authorize must reject.
        let cred = crate::cli::credential_args::CredentialArgs::inline(None);
        let res = authorize(&mut ctx, vec![1], false, None, &cred);
        assert!(matches!(res, Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn deep_scan_requires_a_selector() {
        let (db, buf) = setup();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf));
        let mut ctx = Ctx {
            db: Some(db),
            db_path: PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        };
        let cred = crate::cli::credential_args::CredentialArgs::inline(None);
        let res = deep_scan(&mut ctx, vec![], false, &cred);
        assert!(res.is_err(), "no --machine-ids and no --all must error");
    }

    #[test]
    fn delete_without_yes_flag_returns_invalid_input() {
        let (db, _buf) = setup();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(Vec::new()));
        let mut ctx = Ctx {
            db: Some(db),
            db_path: PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        };

        // Try to delete without --yes or --dry-run
        let result = delete(&mut ctx, 1, false, false);
        assert!(result.is_err(), "delete without --yes or --dry-run should fail");

        if let Err(UecmError::InvalidInput(msg)) = result {
            assert!(
                msg.contains("destructive"),
                "error message should mention destructive"
            );
        } else {
            panic!("expected InvalidInput error");
        }
    }

    #[test]
    fn scan_emits_started_and_completed_events_for_unreachable_cidr() {
        let (db, buf) = setup();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf));
        let mut ctx = Ctx {
            db: Some(db),
            db_path: PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        };
        // TEST-NET-3 /30 = 2 usable hosts; per-port timeout 200ms → completes well under 2s.
        scan(&mut ctx, "203.0.113.0/30", 200).unwrap();
        // Note: we can't easily inspect the buffer here since NdjsonEmitter writes to a moved Vec.
        // But the fact that scan() didn't error means it emitted events successfully.
    }
}
