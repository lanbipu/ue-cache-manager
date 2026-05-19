//! Scans Windows shortcuts, bat files, and services for embedded DDC
//! command-line overrides like -LocalDataCachePath / -SharedDataCachePath.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CmdLineHit {
    pub source: String,
    #[serde(default)]
    pub name: Option<String>,
    pub path: String,
    #[serde(default)]
    pub cmd: Option<String>,
    pub matches: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ScriptResult {
    ok: bool,
    findings: Option<Vec<CmdLineHit>>,
    message: Option<String>,
}

pub fn scan(host: &str, creds: Option<(&str, &str)>) -> UecmResult<Vec<CmdLineHit>> {
    let mut args: Vec<&str> = vec!["-HostName", host];
    if let Some((u, p)) = creds {
        args.extend(["-Username", u, "-Password", p]);
    }
    let r: ScriptResult = powershell::run_json(
        &powershell::script_path("scan-command-line-args.ps1"),
        &args,
    )?;
    if !r.ok {
        return Err(UecmError::OperationFailed(r.message.unwrap_or_default()));
    }
    Ok(r.findings.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn returns_powershell_error_off_windows() {
        let r = scan("HOST", None);
        assert!(matches!(r, Err(UecmError::PowerShell(_))));
    }
}
