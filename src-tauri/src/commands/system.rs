//! System-level Tauri commands (sidecar tests, app metadata).

use crate::core::powershell;
use crate::error::UecmResult;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct EchoResult {
    pub received: String,
    pub timestamp: String,
    pub machine: String,
}

#[tauri::command]
pub fn test_powershell_bridge(message: String) -> UecmResult<EchoResult> {
    let script_path = resolve_script_path("test-echo.ps1");
    powershell::run_json::<EchoResult>(&script_path, &[&message])
}

fn resolve_script_path(name: &str) -> PathBuf {
    // In dev mode, scripts live at ../ps-scripts/ relative to src-tauri/.
    // In production, scripts will be bundled as resources (deferred to a later plan).
    PathBuf::from("..").join("ps-scripts").join(name)
}
