//! Cluster health-check outcome model and pure aggregators.

use crate::data::{self, Db, HealthCheckRun};
use crate::error::{UecmError, UecmResult};
use crate::core::{ini_diagnostics, powershell};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub const HEALTH_CHECK_IDS: &[&str] = &[
    "smb",
    "firewall_445",
    "share_reachable",
    "ntfs",
    "cred_user",
    "cred_system",
    "system_write",
    "ini_consistency",
    "env_vars",
    "pso_precaching",
    "gpu_consistency",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Healthy,
    Warning,
    Critical,
    Na,
    Offline,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckOutcome {
    pub status: CheckStatus,
    pub message: String,
    pub sample: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HealthSummary {
    pub healthy: i64,
    pub warning: i64,
    pub critical: i64,
    pub offline: i64,
    pub unknown: i64,
    pub total: i64,
}

#[derive(Debug, Deserialize)]
struct ReadIniFileResult {
    ok: bool,
    sections: Vec<ReadIniSection>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ReadIniSection {
    keys: Vec<ReadIniKey>,
}

#[derive(Debug, Deserialize)]
struct ReadIniKey {
    name: String,
    value: String,
}

pub fn run_health_check(
    db: &Db,
    machine_ids: &[i64],
    credential_alias: &str,
    project_paths: &[String],
) -> UecmResult<i64> {
    let credential = data::credentials::find_by_alias(db, credential_alias)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("credential alias '{}' not found", credential_alias))
    })?;
    let password = crate::core::credentials::resolve_password(credential_alias)?;
    let scan_run_id = data::scan_runs::insert(db, "health", machine_ids)?;
    for machine_id in machine_ids {
        let machine = data::machines::find_by_id(db, *machine_id)?
            .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;
        let mut results = crate::core::health_probes::run_with_credential(
            &machine.ip,
            &credential.username,
            &password,
        )
        .unwrap_or_else(|err| offline_results(err.to_string()));
        results.insert("ini_consistency".into(), ini_consistency(db, *machine_id)?);
        results.insert(
            "pso_precaching".into(),
            pso_precaching(&machine.ip, &credential.username, &password, project_paths),
        );
        data::health_check_runs::insert(
            db,
            &HealthCheckRun {
                id: None,
                scan_run_id,
                machine_id: *machine_id,
                machine_results: serde_json::to_value(&results)
                    .map_err(|e| UecmError::OperationFailed(e.to_string()))?,
            },
        )?;
    }

    let gpu_results = gpu_consistency(db, machine_ids)?;
    for machine_id in machine_ids {
        let mut latest = data::health_check_runs::latest_for_machine(db, *machine_id)?
            .ok_or_else(|| UecmError::OperationFailed("health row missing after insert".into()))?;
        let mut map: BTreeMap<String, CheckOutcome> = serde_json::from_value(latest.machine_results)
            .map_err(|e| UecmError::OperationFailed(e.to_string()))?;
        map.insert(
            "gpu_consistency".into(),
            gpu_results.get(machine_id).cloned().unwrap_or_else(|| unknown("No GPU inventory row")),
        );
        latest.machine_results = serde_json::to_value(map)
            .map_err(|e| UecmError::OperationFailed(e.to_string()))?;
        data::health_check_runs::update_results(db, latest.id.unwrap_or_default(), &latest.machine_results)?;
    }

    let rows = data::health_check_runs::list_for_run(db, scan_run_id)?;
    let summary = summarize_rows(&rows)?;
    data::scan_runs::finish(db, scan_run_id, &serde_json::to_value(summary).unwrap())?;
    Ok(scan_run_id)
}

pub fn summarize_rows(rows: &[HealthCheckRun]) -> UecmResult<HealthSummary> {
    let mut summary = HealthSummary::default();
    for row in rows {
        let outcomes: BTreeMap<String, CheckOutcome> = serde_json::from_value(row.machine_results.clone())
            .map_err(|e| UecmError::OperationFailed(e.to_string()))?;
        for outcome in outcomes.values() {
            summary.total += 1;
            match outcome.status {
                CheckStatus::Healthy => summary.healthy += 1,
                CheckStatus::Warning => summary.warning += 1,
                CheckStatus::Critical => summary.critical += 1,
                CheckStatus::Offline => summary.offline += 1,
                CheckStatus::Unknown | CheckStatus::Na => summary.unknown += 1,
            }
        }
    }
    Ok(summary)
}

pub fn gpu_consistency(db: &Db, machine_ids: &[i64]) -> UecmResult<HashMap<i64, CheckOutcome>> {
    let mut combos: HashMap<(String, String), Vec<i64>> = HashMap::new();
    for machine_id in machine_ids {
        let gpus = data::machine_gpus::list_for_machine(db, *machine_id)?;
        if let Some(gpu) = gpus.first() {
            combos
                .entry((gpu.gpu_model.clone(), gpu.driver_version.clone()))
                .or_default()
                .push(*machine_id);
        }
    }
    let baseline = combos.iter().max_by_key(|(_, ids)| ids.len()).map(|(combo, _)| combo.clone());
    let mut out = HashMap::new();
    for machine_id in machine_ids {
        let gpus = data::machine_gpus::list_for_machine(db, *machine_id)?;
        let Some(gpu) = gpus.first() else {
            out.insert(*machine_id, unknown("No GPU inventory row"));
            continue;
        };
        let combo = (gpu.gpu_model.clone(), gpu.driver_version.clone());
        let status = if Some(&combo) == baseline.as_ref() {
            CheckStatus::Healthy
        } else {
            CheckStatus::Warning
        };
        out.insert(
            *machine_id,
            CheckOutcome {
                status,
                message: format!("{} / driver {}", gpu.gpu_model, gpu.driver_version),
                sample: gpu.gpu_model.clone(),
                remediation: "Align GPU model and driver version across render machines before PSO work.".into(),
            },
        );
    }
    Ok(out)
}

fn ini_consistency(db: &Db, machine_id: i64) -> UecmResult<CheckOutcome> {
    let open = data::ini_findings::list_open_for_machine(db, machine_id)?;
    let critical = open.iter().filter(|f| f.severity == "critical").count();
    let warning = open.iter().filter(|f| f.severity == "warning").count();
    let status = if critical > 0 {
        CheckStatus::Critical
    } else if warning > 0 {
        CheckStatus::Warning
    } else {
        CheckStatus::Healthy
    };
    Ok(CheckOutcome {
        status,
        message: format!("{} critical, {} warning open INI findings", critical, warning),
        sample: "".into(),
        remediation: "Open INI Scanner and resolve critical/warning findings.".into(),
    })
}

fn pso_precaching(host: &str, username: &str, password: &str, project_paths: &[String]) -> CheckOutcome {
    if project_paths.is_empty() {
        return CheckOutcome {
            status: CheckStatus::Na,
            message: "No project path supplied; PSO CVar check skipped.".into(),
            sample: "".into(),
            remediation: "Supply project paths when running Health Check.".into(),
        };
    }

    let file_path = format!("{}\\Config\\ConsoleVariables.ini", trim_slashes(&project_paths[0]));
    match read_ini_file(host, &file_path, username, password) {
        Ok(read) => {
            let values = cvar_values(read);
            pso_precaching_from_values(&file_path, &values)
        }
        Err(err) => CheckOutcome {
            status: CheckStatus::Warning,
            message: format!("Could not read PSO CVar file: {}", err),
            sample: file_path,
            remediation: "Create Config\\ConsoleVariables.ini and set PSO precaching CVars.".into(),
        },
    }
}

fn pso_precaching_from_values(file_path: &str, values: &HashMap<String, String>) -> CheckOutcome {
    let precaching = values.get("r.PSOPrecaching").map(|v| truthy(v)).unwrap_or(false);
    let validation = values
        .get("r.PSOPrecaching.Validation")
        .map(|v| !disabled(v))
        .unwrap_or(false);
    if precaching && validation {
        CheckOutcome {
            status: CheckStatus::Healthy,
            message: "PSO precaching CVars are enabled.".into(),
            sample: file_path.into(),
            remediation: "No action required.".into(),
        }
    } else {
        CheckOutcome {
            status: CheckStatus::Warning,
            message: format!(
                "r.PSOPrecaching={}, r.PSOPrecaching.Validation={}",
                values.get("r.PSOPrecaching").map(String::as_str).unwrap_or("<missing>"),
                values
                    .get("r.PSOPrecaching.Validation")
                    .map(String::as_str)
                    .unwrap_or("<missing>")
            ),
            sample: file_path.into(),
            remediation: "Set r.PSOPrecaching=1 and r.PSOPrecaching.Validation=1 in Config\\ConsoleVariables.ini.".into(),
        }
    }
}

fn read_ini_file(host: &str, file_path: &str, username: &str, password: &str) -> UecmResult<ReadIniFileResult> {
    let result: ReadIniFileResult = powershell::run_json(
        &powershell::script_path("read-ini-file.ps1"),
        &[
            "-HostName",
            host,
            "-FilePath",
            file_path,
            "-Username",
            username,
            "-Password",
            password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(result.message));
    }
    Ok(result)
}

fn cvar_values(read: ReadIniFileResult) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for section in read.sections {
        for key in section.keys {
            out.insert(ini_diagnostics::normalized_key(&key.name).to_string(), key.value);
        }
    }
    out
}

fn truthy(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

fn disabled(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off")
}

fn trim_slashes(value: &str) -> &str {
    value.trim().trim_end_matches(['\\', '/'])
}

fn offline_results(message: String) -> BTreeMap<String, CheckOutcome> {
    let mut out = BTreeMap::new();
    for id in HEALTH_CHECK_IDS {
        out.insert((*id).into(), CheckOutcome {
            status: CheckStatus::Offline,
            message: message.clone(),
            sample: "".into(),
            remediation: "Restore WinRM connectivity and rerun the health check.".into(),
        });
    }
    out
}

fn unknown(message: &str) -> CheckOutcome {
    CheckOutcome {
        status: CheckStatus::Unknown,
        message: message.into(),
        sample: "".into(),
        remediation: "Refresh machine inventory and rerun the health check.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{machine_gpus, machines, open_in_memory, schema, GpuInfo, GpuVendor, Machine};

    #[test]
    fn gpu_consistency_marks_majority_combo_healthy() {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let a = machines::insert(&db, &Machine::new("A", "10.0.0.1")).unwrap();
        let b = machines::insert(&db, &Machine::new("B", "10.0.0.2")).unwrap();
        let c = machines::insert(&db, &Machine::new("C", "10.0.0.3")).unwrap();
        for id in [a, b] {
            machine_gpus::insert(&db, &gpu(id, "RTX 4090", "551.86")).unwrap();
        }
        machine_gpus::insert(&db, &gpu(c, "RTX 3090", "551.86")).unwrap();
        let result = gpu_consistency(&db, &[a, b, c]).unwrap();
        assert_eq!(result[&a].status, CheckStatus::Healthy);
        assert_eq!(result[&c].status, CheckStatus::Warning);
    }

    #[test]
    fn pso_precaching_from_values_reports_healthy_when_required_cvars_enabled() {
        let values = HashMap::from([
            ("r.PSOPrecaching".into(), "1".into()),
            ("r.PSOPrecaching.Validation".into(), "1".into()),
        ]);
        let outcome = pso_precaching_from_values("E:\\Proj\\Config\\ConsoleVariables.ini", &values);
        assert_eq!(outcome.status, CheckStatus::Healthy);
    }

    #[test]
    fn pso_precaching_from_values_warns_when_validation_missing() {
        let values = HashMap::from([("r.PSOPrecaching".into(), "1".into())]);
        let outcome = pso_precaching_from_values("E:\\Proj\\Config\\ConsoleVariables.ini", &values);
        assert_eq!(outcome.status, CheckStatus::Warning);
        assert!(outcome.message.contains("<missing>"));
    }

    #[test]
    fn offline_results_returns_only_declared_health_checks() {
        let results = offline_results("offline".into());
        assert_eq!(results.len(), HEALTH_CHECK_IDS.len());
        assert!(!results.contains_key("winrm"));
        assert!(results.contains_key("system_write"));
    }

    fn gpu(machine_id: i64, model: &str, driver: &str) -> GpuInfo {
        GpuInfo {
            id: None,
            machine_id,
            gpu_model: model.into(),
            driver_version: driver.into(),
            vendor: GpuVendor::Nvidia,
            vram_mb: Some(24576),
        }
    }
}
