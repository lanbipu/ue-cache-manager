//! Remote path reachability probe.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PathProbeResult {
    ok: bool,
    exists: bool,
    message: String,
}

pub fn exists_with_credential(host: &str, path: &str, username: &str, password: &str) -> UecmResult<bool> {
    let result: PathProbeResult = powershell::run_json(
        &powershell::script_path("test-remote-path.ps1"),
        &[
            "-HostName",
            host,
            "-Path",
            path,
            "-Username",
            username,
            "-Password",
            password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(result.message));
    }
    Ok(result.exists)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn exists_with_credential_returns_powershell_error_on_non_windows() {
        let result = exists_with_credential("RENDER-01", "\\\\HOST\\DDC", "admin", "p@ss");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
