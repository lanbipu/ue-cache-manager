//! Path B (remote PsExec bootstrap) preflight check.
//!
//! Tests whether a target host can be reached + authenticated against, and
//! whether PsExec can register a temporary SCM service there. Used by the
//! `uecm-cli winrm preflight` command (and any future GUI surface).
//!
//! See `ps-scripts/preflight-path-b.ps1` for the actual probe sequence.

use crate::core::powershell;
use crate::error::UecmResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct PreflightStep {
    pub name: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct PreflightResult {
    pub ok: bool,
    pub verdict: String,
    pub reason: String,
    pub results: Vec<PreflightStep>,
}

/// Run the Path B preflight against `host`. When `with_psexec_probe` is true,
/// the script will actually invoke PsExec64 with a noop command and write two
/// entries (service install + remove) to the target machine's Event Log.
pub fn preflight_path_b(
    host: &str,
    user: &str,
    pass: &str,
    with_psexec_probe: bool,
) -> UecmResult<PreflightResult> {
    let psexec_path = powershell::vendor_path("PsExec64.exe");
    let psexec_str = psexec_path.to_string_lossy().into_owned();

    let mut args: Vec<&str> = vec![
        "-HostName",
        host,
        "-Username",
        user,
        "-Password",
        pass,
        "-PsExecPath",
        &psexec_str,
    ];
    if with_psexec_probe {
        args.push("-WithPsExec");
    }

    let result: PreflightResult =
        powershell::run_json(&powershell::script_path("preflight-path-b.ps1"), &args)?;
    Ok(result)
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use crate::error::UecmError;

    #[test]
    fn preflight_returns_powershell_error_on_non_windows() {
        let result = preflight_path_b("192.168.10.50", "Administrator", "secret", false);
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
