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

/// On Windows: create the directory and set ACLs via icacls locally.
/// On non-Windows (dev/CI): just create the directory (icacls is not available).
#[cfg(windows)]
fn provision_local_cache_dir(local_path: &str, service_account: Option<&str>) -> UecmResult<String> {
    use std::process::Command;
    std::fs::create_dir_all(local_path)
        .map_err(|e| UecmError::OperationFailed(format!("mkdir {}: {}", local_path, e)))?;
    for grant in ["SYSTEM:(OI)(CI)F", "Administrators:(OI)(CI)F"] {
        let status = Command::new("icacls")
            .args([local_path, "/grant", grant, "/T", "/C"])
            .status()
            .map_err(|e| UecmError::OperationFailed(format!("icacls: {}", e)))?;
        if !status.success() {
            return Err(UecmError::OperationFailed(format!("icacls {} failed", grant)));
        }
    }
    if let Some(svc) = service_account {
        let grant = format!("{}:(OI)(CI)F", svc);
        let status = Command::new("icacls")
            .args([local_path, "/grant", &grant, "/T", "/C"])
            .status()
            .map_err(|e| UecmError::OperationFailed(format!("icacls: {}", e)))?;
        if !status.success() {
            return Err(UecmError::OperationFailed(format!("icacls {} failed", grant)));
        }
    }
    Ok(local_path.to_string())
}

#[cfg(not(windows))]
fn provision_local_cache_dir(local_path: &str, _service_account: Option<&str>) -> UecmResult<String> {
    std::fs::create_dir_all(local_path)
        .map_err(|e| UecmError::OperationFailed(format!("mkdir {}: {}", local_path, e)))?;
    Ok(local_path.to_string())
}

pub fn create(
    host: &str,
    local_path: &str,
    service_account: Option<&str>,
    operator: Option<(&str, &str)>,
) -> UecmResult<String> {
    if crate::core::loopback::is_loopback_target(host) {
        return provision_local_cache_dir(local_path, service_account);
    }

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

    #[cfg(not(windows))]
    #[test]
    fn loopback_call_works_off_windows() {
        let tmp = tempfile::tempdir().unwrap();
        let path_str = tmp.path().to_str().unwrap();
        let r = create("localhost", path_str, None, None);
        // On non-Windows the mkdir succeeds (icacls is skipped via cfg)
        assert!(r.is_ok(), "got: {:?}", r);
    }
}
