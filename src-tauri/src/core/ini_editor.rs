//! Single-machine INI section read + key write via PowerShell sidecar.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IniKey {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadResult {
    pub ok: bool,
    pub keys: Vec<IniKey>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct WriteResult {
    pub ok: bool,
    pub backup_path: String,
    pub message: String,
}

pub fn read_section(host: &str, file_path: &str, section: &str) -> UecmResult<Vec<IniKey>> {
    let result: ReadResult = powershell::run_json(
        &powershell::script_path("read-ini-section.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Section", section,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "read INI failed: {}",
            result.message
        )));
    }
    Ok(result.keys)
}

pub fn set_key(
    host: &str,
    file_path: &str,
    section: &str,
    name: &str,
    value: &str,
) -> UecmResult<String> {
    let result: WriteResult = powershell::run_json(
        &powershell::script_path("write-ini-key.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Section", section,
            "-Name", name,
            "-Value", value,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "write INI failed: {}",
            result.message
        )));
    }
    Ok(result.backup_path)
}

/// Same as `read_section`, but authenticates the WinRM session with explicit
/// `username` + `password` instead of inheriting the caller's identity.
pub fn read_section_with_credential(
    host: &str,
    file_path: &str,
    section: &str,
    username: &str,
    password: &str,
) -> UecmResult<Vec<IniKey>> {
    let result: ReadResult = powershell::run_json(
        &powershell::script_path("read-ini-section.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Section", section,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "read INI failed: {}",
            result.message
        )));
    }
    Ok(result.keys)
}

/// Same as `set_key`, but authenticates the WinRM session with explicit
/// `username` + `password` instead of inheriting the caller's identity.
pub fn set_key_with_credential(
    host: &str,
    file_path: &str,
    section: &str,
    name: &str,
    value: &str,
    username: &str,
    password: &str,
) -> UecmResult<String> {
    let result: WriteResult = powershell::run_json(
        &powershell::script_path("write-ini-key.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Section", section,
            "-Name", name,
            "-Value", value,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "write INI failed: {}",
            result.message
        )));
    }
    Ok(result.backup_path)
}

pub fn remove_key_with_credential(
    host: &str,
    file_path: &str,
    section: &str,
    name: &str,
    username: &str,
    password: &str,
) -> UecmResult<String> {
    let result: WriteResult = powershell::run_json(
        &powershell::script_path("write-ini-key.ps1"),
        &[
            "-HostName",
            host,
            "-FilePath",
            file_path,
            "-Section",
            section,
            "-Name",
            name,
            "-Value",
            "",
            "-RemoveKey",
            "-Username",
            username,
            "-Password",
            password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "remove INI key failed: {}",
            result.message
        )));
    }
    Ok(result.backup_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn read_section_returns_powershell_error_on_non_windows() {
        let result = read_section("RENDER-01", "C:\\proj\\Config\\DefaultEngine.ini", "Core.System");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn set_key_returns_powershell_error_on_non_windows() {
        let result = set_key(
            "RENDER-01",
            "C:\\proj\\Config\\DefaultEngine.ini",
            "Core.System",
            "Paths",
            "../Content",
        );
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn read_section_with_credential_returns_powershell_error_on_non_windows() {
        let result = read_section_with_credential(
            "RENDER-01",
            "C:\\proj\\Config\\DefaultEngine.ini",
            "Core.System",
            "admin",
            "p@ss",
        );
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn set_key_with_credential_returns_powershell_error_on_non_windows() {
        let result = set_key_with_credential(
            "RENDER-01",
            "C:\\proj\\Config\\DefaultEngine.ini",
            "Core.System",
            "Paths",
            "../Content",
            "admin",
            "p@ss",
        );
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn remove_key_with_credential_returns_powershell_error_on_non_windows() {
        let result = remove_key_with_credential(
            "RENDER-01",
            "C:\\proj\\Config\\DefaultEngine.ini",
            "Core.System",
            "Paths",
            "admin",
            "p@ss",
        );
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
