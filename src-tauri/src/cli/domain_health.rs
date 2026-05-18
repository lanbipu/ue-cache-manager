//! `uecm-cli health <action>` handlers.

use crate::cli::args::HealthAction;
use crate::cli::credential_args::CredentialArgs;
use crate::cli::output::{EmitSerialize, Event};
use crate::cli::run::Ctx;
use crate::core::{health_check::aggregate_gpu_consistency, health_probes};
use crate::data::{
    credentials as data_credentials, health_check_runs, machine_gpus, machines as data_machines,
    scan_runs, share_configs,
};
use crate::error::UecmResult;
use std::collections::HashMap;

pub fn handle(ctx: &mut Ctx<'_>, action: HealthAction) -> UecmResult<()> {
    match action {
        HealthAction::Run { machine_ids, cred } => run(ctx, &machine_ids, &cred),
        HealthAction::Runs { limit } => list_runs(ctx, limit),
        HealthAction::Results { scan_run_id } => list_results(ctx, scan_run_id),
    }
}

fn run(ctx: &mut Ctx<'_>, machine_ids: &[i64], cred: &CredentialArgs) -> UecmResult<()> {
    let db = ctx.require_db()?.clone();

    // Resolve credentials first (needs DB for alias lookup).
    let resolved_cred = cred.resolve(&db)?;
    let (op_user, op_pass) = match &resolved_cred {
        Some((u, p)) => (u.clone(), p.clone()),
        None => (String::new(), String::new()),
    };

    let total = machine_ids.len() as i64;

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "health_run".into(),
            task_id: None,
            metadata: serde_json::json!({ "machines": total }),
        })
        .ok();

    // Mirror commands::health_check::run_health_check exactly:
    // 1. aggregate GPU consistency across all machines
    // 2. resolve primary share (cluster-wide)
    // 3. per-machine: run probes + derive ini/pso/gpu outcomes, upsert row
    // 4. finish scan_run with summary

    let all_gpus: Vec<machine_gpus::GpuInfo> = {
        let mut acc = Vec::new();
        for &mid in machine_ids {
            acc.extend(machine_gpus::list_for_machine(&db, mid)?);
        }
        acc
    };
    let gpu_report = aggregate_gpu_consistency(&all_gpus);

    let primary_share = share_configs::list_all(&db)
        .unwrap_or_default()
        .into_iter()
        .next();
    let cluster_share_unc = primary_share
        .as_ref()
        .map(|s| s.unc_path.clone())
        .unwrap_or_default();
    let cluster_svc_username = match primary_share
        .as_ref()
        .and_then(|s| s.credential_alias.clone())
    {
        Some(alias) => data_credentials::find_by_alias(&db, &alias)?
            .map(|c| c.username)
            .unwrap_or_else(|| "ddc-svc".to_string()),
        None => "ddc-svc".to_string(),
    };

    let scan_id = scan_runs::insert(&db, "health", machine_ids)?;

    let mut healthy: i64 = 0;
    let mut warning: i64 = 0;
    let mut critical: i64 = 0;
    let mut offline: i64 = 0;
    let mut total_checks: i64 = 0;

    for (idx, &mid) in machine_ids.iter().enumerate() {
        let machine = match data_machines::find_by_id(&db, mid)? {
            Some(m) => m,
            None => continue,
        };

        ctx.emitter
            .emit_event(&Event::ItemStarted {
                item_id: format!("machine:{}", mid),
                index: idx as i64,
                total,
            })
            .ok();

        let cred_opt = if resolved_cred.is_some() {
            Some((op_user.as_str(), op_pass.as_str()))
        } else {
            None
        };

        let probes = match health_probes::run(
            &machine.ip,
            &cluster_share_unc,
            &cluster_svc_username,
            &cluster_share_unc,
            cred_opt,
        ) {
            Ok(map) => map,
            Err(e) => {
                // offline — fill all probe keys with "offline" status, same as UI command
                let mut row: HashMap<String, crate::core::health_check::CheckOutcome> = HashMap::new();
                for k in [
                    "smb",
                    "firewall_445",
                    "share_reachable",
                    "ntfs_perm",
                    "cred_user",
                    "cred_system",
                    "env_vars",
                    "system_write",
                ] {
                    row.insert(
                        k.into(),
                        crate::core::health_check::CheckOutcome {
                            status: "offline".into(),
                            message: e.to_string(),
                            sample: "".into(),
                            remediation: String::new(),
                        },
                    );
                }
                health_check_runs::upsert(
                    &db,
                    scan_id,
                    mid,
                    &serde_json::to_value(&row).unwrap(),
                )?;
                offline += 1;
                total_checks += 8;
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: format!("machine:{}", mid),
                        index: idx as i64,
                        ok: false,
                        message: Some(e.to_string()),
                    })
                    .ok();
                continue;
            }
        };

        // Derived checks (mirrors UI command — ini_consistency, pso_precaching, gpu_consistency).
        // pso_precaching requires project paths which CLI doesn't expose; emit "na" like the
        // UI command does when project_paths is empty.
        let ini_outcome = derive_ini_outcome(&db, mid)?;
        let pso_outcome = crate::core::health_check::CheckOutcome {
            status: "na".into(),
            message: "no project paths supplied via CLI".into(),
            sample: "".into(),
            remediation: String::new(),
        };
        let gpu_outcome = gpu_report
            .outcomes
            .get(&mid)
            .cloned()
            .unwrap_or(crate::core::health_check::CheckOutcome {
                status: "unknown".into(),
                message: "no GPU data".into(),
                sample: "".into(),
                remediation: String::new(),
            });

        let mut row = probes;
        row.insert("ini_consistency".into(), ini_outcome);
        row.insert("pso_precaching".into(), pso_outcome);
        row.insert("gpu_consistency".into(), gpu_outcome);

        let machine_checks = row.len() as i64;
        for v in row.values() {
            total_checks += 1;
            match v.status.as_str() {
                "healthy" => healthy += 1,
                "warning" => warning += 1,
                "critical" => critical += 1,
                "offline" => offline += 1,
                _ => {}
            }
        }

        health_check_runs::upsert(&db, scan_id, mid, &serde_json::to_value(&row).unwrap())?;

        ctx.emitter
            .emit_event(&Event::ItemCompleted {
                item_id: format!("machine:{}", mid),
                index: idx as i64,
                ok: true,
                message: Some(format!("{} checks", machine_checks)),
            })
            .ok();
    }

    let summary_json = serde_json::json!({
        "healthy": healthy,
        "warning": warning,
        "critical": critical,
        "offline": offline,
        "total": total_checks,
    });
    scan_runs::finish(&db, scan_id, &summary_json)?;

    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "scan_run_id": scan_id,
                "machines": total,
                "healthy": healthy,
                "warning": warning,
                "critical": critical,
                "offline": offline,
                "total_checks": total_checks,
            }),
        })
        .ok();

    Ok(())
}

fn derive_ini_outcome(
    db: &crate::data::Db,
    machine_id: i64,
) -> UecmResult<crate::core::health_check::CheckOutcome> {
    use crate::data::ini_findings;
    let recent = scan_runs::list_recent(db, "ini", 1)?;
    let Some(latest) = recent.first() else {
        return Ok(crate::core::health_check::CheckOutcome {
            status: "unknown".into(),
            message: "no INI scan run yet".into(),
            sample: "".into(),
            remediation: String::new(),
        });
    };
    let counts =
        ini_findings::count_by_severity_for_machine(db, latest.id.unwrap(), machine_id)?;
    let status = if counts.critical > 0 {
        "critical"
    } else if counts.warning > 0 {
        "warning"
    } else {
        "healthy"
    };
    Ok(crate::core::health_check::CheckOutcome {
        status: status.into(),
        message: format!(
            "{} critical / {} warning open",
            counts.critical, counts.warning
        ),
        sample: format!("scan_run #{}", latest.id.unwrap()),
        remediation: String::new(),
    })
}

fn list_runs(ctx: &mut Ctx<'_>, limit: i64) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = scan_runs::list_recent(db, "health", limit)?;
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}

fn list_results(ctx: &mut Ctx<'_>, scan_run_id: i64) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = health_check_runs::list_for_run(db, scan_run_id)?;
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}
