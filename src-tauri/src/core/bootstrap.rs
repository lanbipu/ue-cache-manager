//! First-contact remote-management bootstrap.
//!
//! This module covers two paths:
//! - automatic bootstrap through Windows admin channels (`ADMIN$` + PsExec)
//! - manual fallback script text for machines with no remote management entry

use crate::core::powershell;
use crate::error::UecmResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WinrmBootstrapResult {
    pub ok: bool,
    pub method: String,
    pub message: String,
    pub winrm_ok: bool,
    #[serde(default)]
    pub changed: Vec<String>,
    pub manual_script: Option<String>,
}

/// USB/local fallback script. Kept as a compile-time include so the UI can
/// still show the script even if resource lookup is unavailable in tests.
pub fn manual_winrm_script() -> String {
    include_str!("../../../ps-scripts/enable-winrm.ps1").to_string()
}

pub fn enable_winrm_with_psexec(
    host: &str,
    username: &str,
    password: &str,
    enable_local_account_remote_admin: bool,
) -> UecmResult<WinrmBootstrapResult> {
    let psexec = powershell::vendor_path("PsExec64.exe");
    let psexec_str = psexec.to_string_lossy().into_owned();
    let local_script = powershell::script_path("enable-winrm.ps1");
    let local_script_str = local_script.to_string_lossy().into_owned();

    let mut args: Vec<&str> = vec![
        "-HostName",
        host,
        "-Username",
        username,
        "-Password",
        password,
        "-PsExecPath",
        &psexec_str,
        "-LocalScriptPath",
        &local_script_str,
    ];
    if enable_local_account_remote_admin {
        args.push("-EnableLocalAccountRemoteAdmin");
    }

    let mut result: WinrmBootstrapResult =
        powershell::run_json(&powershell::script_path("bootstrap-winrm-remote.ps1"), &args)?;

    if !result.ok {
        result.manual_script = Some(manual_winrm_script());
    }
    Ok(result)
}
