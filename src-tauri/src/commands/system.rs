//! System-level Tauri commands (sidecar tests, app metadata).

use crate::core::powershell;
use crate::error::UecmResult;
use serde::Deserialize;

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct EchoResult {
    pub received: String,
    pub timestamp: String,
    pub machine: String,
}

#[tauri::command]
pub fn test_powershell_bridge(message: String) -> UecmResult<EchoResult> {
    let script_path = powershell::script_path("test-echo.ps1");
    powershell::run_json::<EchoResult>(&script_path, &[&message])
}
