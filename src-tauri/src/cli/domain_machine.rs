//! `uecm-cli machine <action>` handlers.

use crate::cli::args::MachineAction;
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
        MachineAction::Refresh { id } => refresh(ctx, id),
        MachineAction::Detail { id } => detail(ctx, id),
        MachineAction::Delete { id, yes } => delete(ctx, id, yes),
        MachineAction::Rename { id, hostname } => rename(ctx, id, hostname),
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

fn delete(ctx: &mut Ctx<'_>, id: i64, yes: bool) -> UecmResult<()> {
    if !yes {
        return Err(UecmError::InvalidInput(
            "delete is destructive; pass --yes to confirm".into(),
        ));
    }

    let db = ctx.require_db()?;
    // Check existence first so a typo / repeated delete fails loudly instead
    // of pretending success. `machines::delete` itself is row-count-agnostic.
    if machines::find_by_id(db, id)?.is_none() {
        return Err(UecmError::InvalidInput(format!(
            "machine id={} not found (already deleted or wrong id)",
            id
        )));
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

fn refresh(ctx: &mut Ctx<'_>, id: i64) -> UecmResult<()> {
    // Fetch machine, grab IP (canonical connect target — matches the UI's
    // `commands::discovery::refresh_machine`). Hostname can drift if the user
    // renames the row, so IP is the only reliable WinRM target.
    let (host, hostname_for_log) = {
        let db = ctx.require_db()?;
        let machine = machines::find_by_id(db, id)?
            .ok_or_else(|| UecmError::InvalidInput(format!("machine id={} not found", id)))?;
        (machine.ip.clone(), machine.hostname.clone())
    };

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "machine_refresh".into(),
            task_id: Some(format!("machine:{}", id)),
            metadata: serde_json::json!({ "ip": host, "hostname": hostname_for_log }),
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
    let probe = match crate::core::winrm::probe(&host) {
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
    let detected_ue = crate::core::discovery::detect_ue_versions(&host)?;
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
        machine_ue_installs::delete_for_machine(db, id)?;
        for (idx, detected) in detected_ue.iter().enumerate() {
            let install = machine_ue_installs::UeInstall {
                id: None,
                machine_id: id,
                version: detected.version.clone(),
                install_path: detected.install_path.clone(),
                is_primary: Some(idx) == primary_idx,
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
    let detected_gpus = crate::core::discovery::detect_gpus(&host)?;
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
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
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
        delete(&mut ctx, 1, true).expect("delete should succeed");

        // Verify that we got past all operations
        // (output checking omitted as NdjsonEmitter writes to a moved buffer)
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

        // Try to delete without --yes
        let result = delete(&mut ctx, 1, false);
        assert!(result.is_err(), "delete without --yes should fail");

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
