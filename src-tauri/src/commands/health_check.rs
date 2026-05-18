//! Tauri commands for the cluster health check.

use crate::core::credentials as core_credentials;
use crate::core::health_check::{aggregate_gpu_consistency, probe_tcp_ports, CheckOutcome};
use crate::core::health_probes;
use crate::core::ini_scanner;
use crate::core::probe_keys;
use crate::data::{
    credentials as data_credentials, ini_findings, machine_gpus,
    machines as data_machines, scan_runs, share_configs,
    health_check_runs, Db,
};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Serialize, Clone)]
pub struct HealthProgressEvent {
    pub scan_run_id: i64,
    pub machine_id: i64,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthRunSummary {
    pub scan_run_id: i64,
    pub healthy: i64,
    pub warning: i64,
    pub critical: i64,
    pub offline: i64,
    /// `na` outcomes — probe could not run (no creds, no share configured, etc).
    /// Separated from `healthy`/`warning`/`critical` so the UI does not inflate green.
    pub skipped: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunHealthCheckRequest {
    pub machine_ids: Vec<i64>,
    pub credential_alias: String,
    pub project_paths: Vec<String>,
}

#[tauri::command]
pub fn run_health_check(
    db: State<'_, Db>,
    app: AppHandle,
    request: RunHealthCheckRequest,
) -> UecmResult<HealthRunSummary> {
    let machine_ids = request.machine_ids;
    let credential_alias = request.credential_alias;
    let project_paths_per_machine: HashMap<i64, Vec<String>> = machine_ids
        .iter()
        .map(|machine_id| (*machine_id, request.project_paths.clone()))
        .collect();
    if machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("machine_ids required".into()));
    }
    let cred_row = data_credentials::find_by_alias(&db, &credential_alias)?
        .ok_or_else(|| UecmError::InvalidInput(format!("credential '{}' not found", credential_alias)))?;
    let password = core_credentials::resolve_password(&credential_alias)?;

    let scan_id = scan_runs::insert(&db, "health", &machine_ids)?;

    let all_gpus: Vec<machine_gpus::GpuInfo> = {
        let mut acc = Vec::new();
        for &mid in &machine_ids {
            acc.extend(machine_gpus::list_for_machine(&db, mid)?);
        }
        acc
    };
    let gpu_report = aggregate_gpu_consistency(&all_gpus);

    // Cluster-wide DDC share to validate from every machine. Fix codex P1:
    // previously this used `share_configs::find_by_host(mid)` per row, which
    // returned shares hosted ON the machine being checked, so client machines
    // probed an empty UNC and reported `na` for share_reachable / ntfs_perm /
    // system_write instead of validating their access to the configured share.
    let primary_share = share_configs::list_all(&db).unwrap_or_default()
        .into_iter().next();
    let cluster_share_unc = primary_share.as_ref().map(|s| s.unc_path.clone()).unwrap_or_default();
    // The share's stored `credential_alias` is the UECM alias (e.g.
    // `UECM:share:HOST-A:ddc-svc`); resolve it to the actual Windows account
    // name (`ddc-svc`) before passing to health-probes.ps1.
    let cluster_svc_username = match primary_share.as_ref().and_then(|s| s.credential_alias.clone()) {
        Some(alias) => data_credentials::find_by_alias(&db, &alias)?
            .map(|c| c.username)
            .unwrap_or_else(|| "ddc-svc".to_string()),
        None => "ddc-svc".to_string(),
    };

    let mut summary = HealthRunSummary {
        scan_run_id: scan_id,
        healthy: 0, warning: 0, critical: 0, offline: 0, skipped: 0,
        total: 0,
    };

    // Single Tokio runtime for L1 TCP probes — the Tauri command is sync.
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| UecmError::OperationFailed(e.to_string()))?;

    for &mid in &machine_ids {
        let machine = match data_machines::find_by_id(&db, mid)? {
            Some(m) => m,
            None => continue,
        };

        let share_unc = cluster_share_unc.clone();
        let svc_username = cluster_svc_username.clone();
        let expected_shared = share_unc.clone();

        let probes = match health_probes::run(
            &machine.ip, &share_unc, &svc_username, &expected_shared,
            Some((&cred_row.username, &password)),
        ) {
            Ok(map) => map,
            Err(e) => {
                let _ = app.emit("health-progress", HealthProgressEvent {
                    scan_run_id: scan_id, machine_id: mid, done: true,
                    error: Some(e.to_string()),
                });
                // Offline branch: fill registry keys with `offline`, then inject L1
                // (operator may have lost WinRM but kept TCP visibility).
                let mut row = HashMap::<String, CheckOutcome>::new();
                for k in probe_keys::offline_probe_keys() {
                    row.insert(k.into(), CheckOutcome {
                        status: "offline".into(),
                        message: e.to_string(),
                        sample: "".into(),
                        remediation: "Bring the host online (verify network + WinRM) before retrying.".into(),
                    });
                }
                let l1 = rt.block_on(probe_tcp_ports(&machine.ip, 1000));
                for (k, v) in l1 { row.insert(k, v); }
                tally_summary(&mut summary, &row);
                health_check_runs::upsert(&db, scan_id, mid, &serde_json::to_value(&row).unwrap())?;
                continue;
            }
        };

        let ini_outcome = derive_ini_outcome(&db, mid)?;

        let pso_outcome = derive_pso_cvar_outcome(
            &machine.ip, &cred_row.username, &password,
            project_paths_per_machine.get(&mid).cloned().unwrap_or_default(),
        );

        let gpu_outcome = gpu_report.outcomes.get(&mid).cloned()
            .unwrap_or(CheckOutcome { status: "unknown".into(), message: "no GPU data".into(), sample: "".into(), remediation: String::new() });

        let mut row: HashMap<String, CheckOutcome> = probes;
        row.insert("ini_consistency".into(), ini_outcome);
        row.insert("pso_precaching".into(), pso_outcome);
        row.insert("gpu_consistency".into(), gpu_outcome);

        // L1 ports — creds-independent, always run.
        let l1 = rt.block_on(probe_tcp_ports(&machine.ip, 1000));
        for (k, v) in l1 { row.insert(k, v); }

        tally_summary(&mut summary, &row);
        health_check_runs::upsert(&db, scan_id, mid, &serde_json::to_value(&row).unwrap())?;
        let _ = app.emit("health-progress", HealthProgressEvent {
            scan_run_id: scan_id, machine_id: mid, done: true, error: None,
        });
    }

    let summary_json = json!({
        "healthy": summary.healthy, "warning": summary.warning,
        "critical": summary.critical, "offline": summary.offline,
        "skipped": summary.skipped, "total": summary.total,
    });
    scan_runs::finish(&db, scan_id, &summary_json)?;
    Ok(summary)
}

/// Tally one machine's per-key outcomes into the run summary.
/// `na` is segregated into `skipped` (does NOT count toward healthy/warning/critical).
/// Mirrors the `Counters::tally` logic in `cli/domain_health.rs` so UI and CLI agree.
fn tally_summary(summary: &mut HealthRunSummary, row: &HashMap<String, CheckOutcome>) {
    for v in row.values() {
        match v.status.as_str() {
            "healthy"  => { summary.healthy  += 1; summary.total += 1; }
            "warning"  => { summary.warning  += 1; summary.total += 1; }
            "critical" => { summary.critical += 1; summary.total += 1; }
            "offline"  => { summary.offline  += 1; summary.total += 1; }
            "na"       => { summary.skipped  += 1; }
            _          => {}
        }
    }
}

fn derive_ini_outcome(db: &Db, machine_id: i64) -> UecmResult<CheckOutcome> {
    let recent = scan_runs::list_recent(db, "ini", 1)?;
    let Some(latest) = recent.first() else {
        return Ok(CheckOutcome { status: "unknown".into(), message: "no INI scan run yet".into(), sample: "".into(), remediation: String::new() });
    };
    let counts = ini_findings::count_by_severity_for_machine(db, latest.id.unwrap(), machine_id)?;
    let status = if counts.critical > 0 { "critical" }
        else if counts.warning > 0 { "warning" }
        else { "healthy" };
    Ok(CheckOutcome {
        status: status.into(),
        message: format!("{} critical / {} warning open", counts.critical, counts.warning),
        sample: format!("scan_run #{}", latest.id.unwrap()),
        remediation: String::new(),
    })
}

fn derive_pso_cvar_outcome(
    host: &str,
    username: &str,
    password: &str,
    project_roots: Vec<String>,
) -> CheckOutcome {
    if project_roots.is_empty() {
        return CheckOutcome {
            status: "na".into(),
            message: "no project paths supplied".into(),
            sample: "".into(),
            remediation: String::new(),
        };
    }
    let target = ini_scanner::TargetFile {
        path: format!("{}\\Config\\ConsoleVariables.ini", project_roots[0].trim_end_matches('\\')),
        category: crate::core::ini_diagnostics::Category::Project,
    };
    let parsed = match ini_scanner::read_file(host, &target, Some((username, password))) {
        Ok(Some(pf)) => pf,
        Ok(None) => return CheckOutcome { status: "warning".into(), message: "ConsoleVariables.ini missing".into(), sample: target.path, remediation: String::new() },
        Err(e) => return CheckOutcome { status: "offline".into(), message: e.to_string(), sample: target.path, remediation: String::new() },
    };
    let cvar_value = parsed.sections.iter()
        .flat_map(|s| s.keys.iter())
        .find(|k| k.name.eq_ignore_ascii_case("r.PSOPrecaching"))
        .map(|k| k.value.clone());
    match cvar_value.as_deref() {
        Some("1") => CheckOutcome { status: "healthy".into(), message: "r.PSOPrecaching=1".into(), sample: parsed.path, remediation: String::new() },
        Some(other) => CheckOutcome { status: "warning".into(), message: format!("r.PSOPrecaching={}", other), sample: parsed.path, remediation: String::new() },
        None => CheckOutcome { status: "warning".into(), message: "r.PSOPrecaching not set".into(), sample: parsed.path, remediation: String::new() },
    }
}

#[tauri::command]
pub fn list_recent_health_runs(db: State<'_, Db>, limit: i64) -> UecmResult<Vec<scan_runs::ScanRun>> {
    scan_runs::list_recent(&db, "health", limit)
}

#[tauri::command]
pub fn list_health_results_for_run(db: State<'_, Db>, scan_run_id: i64) -> UecmResult<Vec<health_check_runs::HealthCheckRow>> {
    health_check_runs::list_for_run(&db, scan_run_id)
}
