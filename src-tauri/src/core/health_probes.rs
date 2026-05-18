//! WinRM dispatch for `health-probes.ps1`.

use crate::core::{loopback, powershell};
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;
use std::collections::HashMap;
use super::health_check::CheckOutcome;

#[derive(Debug, Deserialize)]
struct ProbeResult {
    pub ok: bool,
    #[serde(default)]
    pub results: HashMap<String, CheckOutcome>,
    #[serde(default)]
    pub message: String,
}

pub fn run(
    host: &str,
    share_unc: &str,
    svc_username: &str,
    expected_shared_path: &str,
    expected_local_path: &str,
    cred: Option<(&str, &str)>,
) -> UecmResult<HashMap<String, CheckOutcome>> {
    let mut args: Vec<String> = vec![
        "-HostName".into(), host.into(),
        "-ShareUnc".into(), share_unc.into(),
        "-SvcUsername".into(), svc_username.into(),
        "-ExpectedSharedDataCachePath".into(), expected_shared_path.into(),
        "-ExpectedLocalDataCachePath".into(), expected_local_path.into(),
    ];
    if loopback::is_loopback_target(host) {
        // Loopback runs the probe scriptblock inside the current PowerShell
        // process — switching to an admin token mid-process is not safe, so
        // any explicit credential is intentionally dropped here. SYSTEM-context
        // probes (PsExec -s) still succeed when UECM itself was launched
        // elevated. See docs/.../uecm-plan-4-followup-scan-ux.md for an
        // out-of-process elevation path.
        let _ = cred;
        args.push("-Local".into());
    } else if let Some((u, p)) = cred {
        args.push("-Username".into()); args.push(u.into());
        args.push("-Password".into()); args.push(p.into());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: ProbeResult = powershell::run_json(
        &powershell::script_path("health-probes.ps1"),
        &arg_refs,
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!("health-probes failed: {}", result.message)));
    }
    Ok(result.results)
}
