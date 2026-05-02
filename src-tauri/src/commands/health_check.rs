//! Tauri commands for cluster health checks.

use crate::core::health_check;
use crate::data::{health_check_runs, scan_runs, Db, HealthCheckRun, ScanRun};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunHealthCheckRequest {
    pub machine_ids: Vec<i64>,
    pub credential_alias: String,
    pub project_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunHealthCheckResponse {
    pub scan_run_id: i64,
    pub results: Vec<HealthCheckRun>,
}

#[tauri::command]
pub fn run_health_check(
    db: State<'_, Db>,
    request: RunHealthCheckRequest,
) -> UecmResult<RunHealthCheckResponse> {
    if request.machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("machine_ids cannot be empty".into()));
    }
    let scan_run_id = health_check::run_health_check(
        &db,
        &request.machine_ids,
        &request.credential_alias,
        &request.project_paths,
    )?;
    Ok(RunHealthCheckResponse {
        scan_run_id,
        results: health_check_runs::list_for_run(&db, scan_run_id)?,
    })
}

#[tauri::command]
pub fn list_health_check_runs(db: State<'_, Db>, limit: i64) -> UecmResult<Vec<ScanRun>> {
    scan_runs::list_recent(&db, "health", limit)
}

#[tauri::command]
pub fn list_health_results_for_run(
    db: State<'_, Db>,
    scan_run_id: i64,
) -> UecmResult<Vec<HealthCheckRun>> {
    health_check_runs::list_for_run(&db, scan_run_id)
}
