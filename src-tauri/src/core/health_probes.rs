//! WinRM dispatch for `health-probes.ps1`.

use crate::core::powershell;
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
    cred: Option<(&str, &str)>,
) -> UecmResult<HashMap<String, CheckOutcome>> {
    let mut args: Vec<String> = vec![
        "-HostName".into(), host.into(),
        "-ShareUnc".into(), share_unc.into(),
        "-SvcUsername".into(), svc_username.into(),
        "-ExpectedSharedDataCachePath".into(), expected_shared_path.into(),
    ];
    if let Some((u, p)) = cred {
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
