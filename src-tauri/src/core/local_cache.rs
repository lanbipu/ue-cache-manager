//! Provision a local DDC directory on a remote host: New-Item + icacls.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CreateResult {
    ok: bool,
    message: String,
    path: Option<String>,
}

pub fn create(
    host: &str,
    local_path: &str,
    service_account: Option<&str>,
    operator: Option<(&str, &str)>,
) -> UecmResult<String> {
    let mut args: Vec<&str> = vec!["-HostName", host, "-LocalPath", local_path];
    if let Some(sa) = service_account {
        args.extend(["-ServiceAccount", sa]);
    }
    if let Some((u, p)) = operator {
        args.extend(["-Username", u, "-Password", p]);
    }
    let r: CreateResult = powershell::run_json(
        &powershell::script_path("create-local-cache-dir.ps1"),
        &args,
    )?;
    if !r.ok {
        return Err(UecmError::OperationFailed(r.message));
    }
    Ok(r.path.unwrap_or(r.message))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(windows))]
    #[test]
    fn returns_powershell_error_off_windows() {
        let r = create("HOST", r"D:\UE-DDC-Local", None, None);
        assert!(matches!(r, Err(UecmError::PowerShell(_))));
    }
}
