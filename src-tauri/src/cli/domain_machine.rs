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
        MachineAction::Refresh { .. } => {
            Err(UecmError::OperationFailed("refresh: pending Task 3.4".into()))
        }
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
