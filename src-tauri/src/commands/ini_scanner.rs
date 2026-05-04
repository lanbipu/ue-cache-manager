//! Tauri commands for INI diagnostics.

use crate::core::{ini_apply, ini_scanner};
use crate::data::{ini_findings, scan_runs, Db, IniFinding, ScanRun};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanInisRequest {
    pub machine_ids: Vec<i64>,
    pub credential_alias: String,
    pub project_paths: Vec<String>,
    pub user_profile_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanInisResponse {
    pub scan_run_id: i64,
    pub summary: ini_scanner::IniScanSummary,
    pub findings: Vec<IniFinding>,
}

#[tauri::command]
pub fn scan_inis(db: State<'_, Db>, request: ScanInisRequest) -> UecmResult<ScanInisResponse> {
    scan_inis_inner(&db, request)
}

pub fn scan_inis_inner(db: &Db, request: ScanInisRequest) -> UecmResult<ScanInisResponse> {
    if request.machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("machine_ids cannot be empty".into()));
    }
    let summary = ini_scanner::run_scan(
        db,
        &request.machine_ids,
        &request.credential_alias,
        &request.project_paths,
        request.user_profile_path.as_deref(),
    )?;
    let findings = ini_findings::list_for_run(db, summary.scan_run_id)?;
    Ok(ScanInisResponse {
        scan_run_id: summary.scan_run_id,
        summary,
        findings,
    })
}

#[tauri::command]
pub fn verify_pso_precaching(
    db: State<'_, Db>,
    request: ScanInisRequest,
) -> UecmResult<ScanInisResponse> {
    verify_pso_precaching_inner(&db, request)
}

pub fn verify_pso_precaching_inner(db: &Db, request: ScanInisRequest) -> UecmResult<ScanInisResponse> {
    if request.project_paths.is_empty() {
        return Err(UecmError::InvalidInput(
            "project_paths cannot be empty for PSO precaching verification".into(),
        ));
    }
    scan_inis_inner(db, request)
}

#[tauri::command]
pub fn list_scan_runs(db: State<'_, Db>, scan_type: String, limit: i64) -> UecmResult<Vec<ScanRun>> {
    scan_runs::list_recent(&db, &scan_type, limit)
}

#[tauri::command]
pub fn list_findings(db: State<'_, Db>, scan_run_id: i64) -> UecmResult<Vec<IniFinding>> {
    ini_findings::list_for_run(&db, scan_run_id)
}

#[tauri::command]
pub fn get_finding(db: State<'_, Db>, finding_id: i64) -> UecmResult<Option<IniFinding>> {
    ini_findings::find_by_id(&db, finding_id)
}

#[tauri::command]
pub fn apply_finding(
    db: State<'_, Db>,
    finding_id: i64,
    credential_alias: String,
) -> UecmResult<ini_apply::ApplyFindingResult> {
    ini_apply::apply(&db, finding_id, &credential_alias)
}

#[tauri::command]
pub fn skip_finding(db: State<'_, Db>, finding_id: i64) -> UecmResult<()> {
    ini_findings::mark_skipped(&db, finding_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{machines, open_in_memory, schema, Machine};

    #[cfg(not(windows))]
    #[test]
    fn apply_finding_returns_invalid_input_for_missing_finding() {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let _ = machines::insert(&db, &Machine::new("RENDER-01", "192.168.10.21")).unwrap();
        let result = ini_apply::apply(&db, 999, "UECM:winrm:RENDER-01");
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn verify_pso_precaching_requires_project_paths() {
        let db = open_in_memory().unwrap();
        let result = verify_pso_precaching_inner(
            &db,
            ScanInisRequest {
                machine_ids: vec![1],
                credential_alias: "missing".into(),
                project_paths: Vec::new(),
                user_profile_path: None,
            },
        );
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }
}
