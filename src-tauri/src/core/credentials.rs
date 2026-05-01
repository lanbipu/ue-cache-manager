//! Wraps the cred-{set,delete,list}.ps1 sidecar scripts. Stores the alias +
//! display username in SQLite (via `data::credentials`); the password lives
//! in Windows Credential Manager and is never read back into our process.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CmdKeyResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CmdKeyAlias {
    pub alias: String,
}

pub fn store(alias: &str, username: &str, password: &str) -> UecmResult<()> {
    let result: CmdKeyResult = powershell::run_json(
        &powershell::script_path("cred-set.ps1"),
        &[
            "-Alias", alias,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "cred-set failed: {}",
            result.message
        )));
    }
    Ok(())
}

pub fn delete(alias: &str) -> UecmResult<()> {
    let result: CmdKeyResult = powershell::run_json(
        &powershell::script_path("cred-delete.ps1"),
        &["-Alias", alias],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "cred-delete failed: {}",
            result.message
        )));
    }
    Ok(())
}

pub fn list_uecm_aliases() -> UecmResult<Vec<String>> {
    let result: Vec<CmdKeyAlias> = powershell::run_json(
        &powershell::script_path("cred-list.ps1"),
        &[],
    )?;
    Ok(result.into_iter().map(|c| c.alias).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn store_returns_powershell_error_on_non_windows() {
        let result = store("UECM:winrm:HOST", "admin", "p@ss");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn delete_returns_powershell_error_on_non_windows() {
        let result = delete("UECM:winrm:HOST");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn list_returns_powershell_error_on_non_windows() {
        let result = list_uecm_aliases();
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
