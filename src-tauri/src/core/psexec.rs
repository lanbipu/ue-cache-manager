//! Wraps `inject-system-credential.ps1`. The PS script forwards itself over
//! WinRM to the target client, then runs PsExec64 -s -i to drop into the
//! SYSTEM context and stores a host-specific cmdkey entry so SYSTEM-context
//! services (e.g. UE engine) can transparently reach the share.
//!
//! `PsExec64.exe` is resolved via `powershell::vendor_path` and forwarded as
//! `-PsExecPath` so the PS script can copy + execute it on the client.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct InjectScriptResult {
    ok: bool,
    message: String,
}

pub fn inject_system_credential(
    client_host: &str,
    target_host: &str,
    svc_user: &str,
    svc_pass: &str,
    operator_user: Option<&str>,
    operator_pass: Option<&str>,
) -> UecmResult<String> {
    let psexec = powershell::vendor_path("PsExec64.exe");
    let psexec_str = psexec.to_string_lossy().into_owned();
    let mut args: Vec<&str> = vec![
        "-ClientHostName", client_host,
        "-TargetHost", target_host,
        "-SvcUsername", svc_user,
        "-SvcPassword", svc_pass,
        "-PsExecPath", &psexec_str,
    ];
    if let (Some(u), Some(p)) = (operator_user, operator_pass) {
        args.extend(["-Username", u, "-Password", p]);
    }
    let result: InjectScriptResult = powershell::run_json(
        &powershell::script_path("inject-system-credential.ps1"),
        &args,
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "SYSTEM credential injection failed: {}",
            result.message
        )));
    }
    Ok(result.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn inject_returns_powershell_error_on_non_windows() {
        let result = inject_system_credential("CLIENT", "HOST", "ddc-svc", "p", None, None);
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
