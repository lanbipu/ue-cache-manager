//! Single-machine environment variable read/write via PowerShell sidecar.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SetResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct GetResult {
    pub ok: bool,
    pub value: Option<String>,
    pub message: String,
}

pub fn set(host: &str, name: &str, value: &str) -> UecmResult<()> {
    let result: SetResult = powershell::run_json(
        &powershell::script_path("setx-machine.ps1"),
        &[
            "-HostName", host,
            "-Name", name,
            "-Value", value,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "set env var failed: {}",
            result.message
        )));
    }
    Ok(())
}

pub fn get(host: &str, name: &str) -> UecmResult<Option<String>> {
    let result: GetResult = powershell::run_json(
        &powershell::script_path("getx-machine.ps1"),
        &[
            "-HostName", host,
            "-Name", name,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "get env var failed: {}",
            result.message
        )));
    }
    Ok(result.value)
}

/// Same as `set`, but authenticates the WinRM session with explicit
/// `username` + `password` instead of inheriting the caller's identity.
pub fn set_with_credential(
    host: &str,
    name: &str,
    value: &str,
    username: &str,
    password: &str,
) -> UecmResult<()> {
    let result: SetResult = powershell::run_json(
        &powershell::script_path("setx-machine.ps1"),
        &[
            "-HostName", host,
            "-Name", name,
            "-Value", value,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "set env var failed: {}",
            result.message
        )));
    }
    Ok(())
}

/// Same as `get`, but authenticates the WinRM session with explicit
/// `username` + `password` instead of inheriting the caller's identity.
pub fn get_with_credential(
    host: &str,
    name: &str,
    username: &str,
    password: &str,
) -> UecmResult<Option<String>> {
    let result: GetResult = powershell::run_json(
        &powershell::script_path("getx-machine.ps1"),
        &[
            "-HostName", host,
            "-Name", name,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "get env var failed: {}",
            result.message
        )));
    }
    Ok(result.value)
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn set_returns_powershell_error_on_non_windows() {
        let result = set("RENDER-01", "UE-SharedDataCachePath", "\\\\HOST\\DDC");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[test]
    fn get_returns_powershell_error_on_non_windows() {
        let result = get("RENDER-01", "UE-SharedDataCachePath");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[test]
    fn set_with_credential_returns_powershell_error_on_non_windows() {
        let result = set_with_credential(
            "RENDER-01",
            "UE-SharedDataCachePath",
            "\\\\HOST\\DDC",
            "admin",
            "p@ss",
        );
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[test]
    fn get_with_credential_returns_powershell_error_on_non_windows() {
        let result = get_with_credential("RENDER-01", "UE-SharedDataCachePath", "admin", "p@ss");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
