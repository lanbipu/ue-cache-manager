//! Windows PowerShell wrapper for per-machine health probes.

use crate::core::{health_check::CheckOutcome, powershell};
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct ProbeResult {
    ok: bool,
    results: BTreeMap<String, CheckOutcome>,
    message: String,
}

pub fn run_with_credential(
    host: &str,
    username: &str,
    password: &str,
) -> UecmResult<BTreeMap<String, CheckOutcome>> {
    let result: ProbeResult = powershell::run_json(
        &powershell::script_path("health-probes.ps1"),
        &[
            "-HostName", host,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(result.message));
    }
    Ok(result.results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn run_with_credential_returns_powershell_error_on_non_windows() {
        let result = run_with_credential("RENDER-01", "admin", "p@ss");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
