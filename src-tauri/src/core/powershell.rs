//! PowerShell sidecar invocation. On Windows, runs powershell.exe with
//! provided script + args, captures stdout, returns parsed JSON result.
//! On non-Windows, returns an error (sidecar is Windows-only).

use crate::error::{UecmError, UecmResult};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;

/// Resolve a sidecar script name to its on-disk path. Convention: scripts live
/// in `ps-scripts/` one level above the cargo package root (so the cwd is
/// `src-tauri/` at runtime + during tests).
pub fn script_path(name: &str) -> PathBuf {
    Path::new("..").join("ps-scripts").join(name)
}

/// Load a sidecar script's text. Used when the script body is forwarded over
/// WinRM via `core::winrm::invoke*` rather than executed locally.
pub fn read_script(name: &str) -> UecmResult<String> {
    Ok(fs::read_to_string(script_path(name))?)
}

#[derive(Debug)]
pub struct ScriptResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Run a .ps1 script with the given arguments. Returns raw output.
pub fn run_script(script_path: &Path, args: &[&str]) -> UecmResult<ScriptResult> {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("powershell.exe");
        cmd.arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(script_path);
        for arg in args {
            cmd.arg(arg);
        }
        let output = cmd.output().map_err(|e| {
            UecmError::PowerShell(format!("failed to spawn powershell.exe: {}", e))
        })?;
        Ok(ScriptResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (script_path, args);
        Err(UecmError::PowerShell(
            "PowerShell sidecar is Windows-only".to_string(),
        ))
    }
}

/// Run a script and parse stdout as JSON of type T.
pub fn run_json<T: DeserializeOwned>(script_path: &Path, args: &[&str]) -> UecmResult<T> {
    let result = run_script(script_path, args)?;
    if result.exit_code != 0 {
        return Err(UecmError::PowerShell(format!(
            "script exited with code {}: {}",
            result.exit_code, result.stderr
        )));
    }
    serde_json::from_str(&result.stdout).map_err(|e| {
        UecmError::PowerShell(format!("failed to parse JSON output: {} (stdout: {})", e, result.stdout))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct EchoOutput {
        received: String,
        machine: String,
    }

    #[cfg(windows)]
    #[test]
    fn test_echo_script_returns_parsed_json() {
        let script = Path::new("../ps-scripts/test-echo.ps1");
        let result: EchoOutput = run_json(script, &["world"]).unwrap();
        assert_eq!(result.received, "world");
        assert!(!result.machine.is_empty());
    }

    #[cfg(not(windows))]
    #[test]
    fn run_script_returns_error_on_non_windows() {
        let script = Path::new("../ps-scripts/test-echo.ps1");
        let result = run_script(script, &[]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UecmError::PowerShell(_)));
    }
}
