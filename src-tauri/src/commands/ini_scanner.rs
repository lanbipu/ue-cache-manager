//! Tauri commands for the INI scanner: dispatch a scan, list findings,
//! apply / skip a single finding.

use crate::core::credentials as core_credentials;
use crate::core::ini_apply::{self, ApplyContext};
use crate::core::ini_diagnostics::EnvVarState;
use crate::core::ini_scanner::{self, ScanInputs};
use crate::core::env_vars;
use crate::data::{
    credentials as data_credentials, ini_findings, machine_ue_installs,
    machines as data_machines, scan_runs, Db, IniFinding,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use serde_json::json;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ScanRunSummary {
    pub scan_run_id: i64,
    pub critical: i64,
    pub warning: i64,
    pub healthy: i64,
}

#[tauri::command]
pub fn scan_inis(
    db: State<'_, Db>,
    machine_ids: Vec<i64>,
    project_paths_per_machine: std::collections::HashMap<i64, Vec<String>>,
    user_profile: String,
    credential_alias: String,
) -> UecmResult<ScanRunSummary> {
    if machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("machine_ids must not be empty".into()));
    }
    let cred_row = data_credentials::find_by_alias(&db, &credential_alias)?
        .ok_or_else(|| UecmError::InvalidInput(format!("credential alias '{}' not found", credential_alias)))?;
    let password = core_credentials::resolve_password(&credential_alias)?;
    let scan_id = scan_runs::insert(&db, "ini", &machine_ids)?;

    let mut total_critical = 0i64;
    let mut total_warning = 0i64;
    let mut total_healthy = 0i64;

    for &mid in &machine_ids {
        let machine = data_machines::find_by_id(&db, mid)?
            .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", mid)))?;
        let installs_rows = machine_ue_installs::list_for_machine(&db, mid)?;
        let installs: Vec<(String, String)> = installs_rows.into_iter()
            .map(|i| (i.version, i.install_path)).collect();
        let project_roots: Vec<String> = project_paths_per_machine.get(&mid).cloned().unwrap_or_default();

        let mut env_state = EnvVarState::default();
        env_state.shared_data_cache_path = env_vars::get_with_credential(
            &machine.ip, "UE-SharedDataCachePath", &cred_row.username, &password,
        ).ok().flatten();
        env_state.local_data_cache_path = env_vars::get_with_credential(
            &machine.ip, "UE-LocalDataCachePath", &cred_row.username, &password,
        ).ok().flatten();

        let inputs = ScanInputs {
            host: &machine.ip,
            credential: Some((&cred_row.username, &password)),
            installs: &installs,
            user_profile: &user_profile,
            project_roots: &project_roots,
            env_state,
        };

        let findings = ini_scanner::scan_machine(&inputs)?;
        for f in findings {
            let row = IniFinding {
                id: None,
                scan_run_id: scan_id,
                machine_id: mid,
                rule_id: f.rule_id,
                severity: f.severity.as_str().into(),
                category: f.category.as_str().into(),
                file_path: f.file_path,
                section: f.section,
                key_name: f.key_name,
                line_number: f.line_number,
                snippet_before: f.snippet_before,
                snippet_after: f.snippet_after,
                recommended_action: f.recommended_action.as_str().into(),
                recommended_value: f.recommended_value,
                symptom: f.symptom,
                rationale: f.rationale,
                fixed_at: None,
                skipped_at: None,
            };
            match row.severity.as_str() {
                "critical" => total_critical += 1,
                "warning" => total_warning += 1,
                "healthy" => total_healthy += 1,
                _ => {}
            }
            ini_findings::insert(&db, &row)?;
        }
    }

    let summary = json!({
        "critical": total_critical,
        "warning": total_warning,
        "healthy": total_healthy,
    });
    scan_runs::finish(&db, scan_id, &summary)?;
    Ok(ScanRunSummary {
        scan_run_id: scan_id,
        critical: total_critical,
        warning: total_warning,
        healthy: total_healthy,
    })
}

#[tauri::command]
pub fn list_findings_for_run(db: State<'_, Db>, scan_run_id: i64) -> UecmResult<Vec<IniFinding>> {
    ini_findings::list_for_run(&db, scan_run_id)
}

#[tauri::command]
pub fn list_recent_ini_runs(db: State<'_, Db>, limit: i64) -> UecmResult<Vec<scan_runs::ScanRun>> {
    scan_runs::list_recent(&db, "ini", limit)
}

#[tauri::command]
pub fn apply_finding(
    db: State<'_, Db>,
    finding_id: i64,
    credential_alias: String,
) -> UecmResult<String> {
    let f = ini_findings::find_by_id(&db, finding_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("finding {} not found", finding_id)))?;
    let machine = data_machines::find_by_id(&db, f.machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", f.machine_id)))?;
    let cred = data_credentials::find_by_alias(&db, &credential_alias)?
        .ok_or_else(|| UecmError::InvalidInput(format!("credential '{}' not found", credential_alias)))?;
    let password = core_credentials::resolve_password(&credential_alias)?;
    let ctx = ApplyContext { host: &machine.ip, credential: (&cred.username, &password) };
    let backup = ini_apply::apply(&ctx, &f)?;
    ini_findings::mark_fixed(&db, finding_id)?;
    Ok(backup)
}

#[tauri::command]
pub fn skip_finding(db: State<'_, Db>, finding_id: i64) -> UecmResult<()> {
    ini_findings::mark_skipped(&db, finding_id)
}
