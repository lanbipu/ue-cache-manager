//! Single-machine INI section read + key write via PowerShell sidecar.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
        &script_path("read-ini-section.ps1"),
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
        &script_path("write-ini-key.ps1"),
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

fn script_path(name: &str) -> PathBuf {
    Path::new("..").join("ps-scripts").join(name)
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
}
