//! High-level DDC Pak generation helpers.

use crate::core::powershell;
use crate::core::ue_runner::{self, UeRunSpec, UeRunnerBackend};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct PreflightRaw {
    ok: bool,
    #[serde(default)]
    exe_exists: bool,
    #[serde(default)]
    proj_exists: bool,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerifyRaw {
    ok: bool,
    #[serde(default)]
    found: bool,
    #[serde(default)]
    path: String,
    #[serde(default)]
    size: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PakOutput {
    pub path: String,
    pub size_bytes: i64,
}

fn default_extra_args() -> Vec<String> {
    vec![
        "-run=DerivedDataCache".into(),
        "-fill".into(),
        "-DDC=CreatePak".into(),
        "-unattended".into(),
        "-nopause".into(),
        "-nosplash".into(),
    ]
}

pub fn preflight(
    host: &str,
    engine_path: &str,
    project_path: &str,
    user: Option<&str>,
    pass: Option<&str>,
) -> UecmResult<()> {
    let mut args = vec![
        "-HostName".to_string(),
        host.to_string(),
        "-EnginePath".into(),
        engine_path.to_string(),
        "-ProjectPath".into(),
        project_path.to_string(),
    ];
    if let (Some(user), Some(pass)) = (user, pass) {
        args.push("-Username".into());
        args.push(user.into());
        args.push("-Password".into());
        args.push(pass.into());
    }
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: PreflightRaw =
        powershell::run_json(&powershell::script_path("generate-ddc-pak.ps1"), &args_ref)?;
    if !result.ok {
        return Err(UecmError::OperationFailed(
            result.message.unwrap_or_else(|| "preflight failed".into()),
        ));
    }
    if !result.exe_exists {
        return Err(UecmError::InvalidInput(
            "UnrealEditor.exe not found at engine_path".into(),
        ));
    }
    if !result.proj_exists {
        return Err(UecmError::InvalidInput(
            ".uproject not found at project_path".into(),
        ));
    }
    Ok(())
}

pub fn verify_output(
    host: &str,
    project_dir: &str,
    user: Option<&str>,
    pass: Option<&str>,
) -> UecmResult<PakOutput> {
    let mut args = vec![
        "-HostName".to_string(),
        host.to_string(),
        "-ProjectDir".into(),
        project_dir.to_string(),
    ];
    if let (Some(user), Some(pass)) = (user, pass) {
        args.push("-Username".into());
        args.push(user.into());
        args.push("-Password".into());
        args.push(pass.into());
    }
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: VerifyRaw =
        powershell::run_json(&powershell::script_path("verify-pak-output.ps1"), &args_ref)?;
    if !result.ok {
        return Err(UecmError::OperationFailed(
            result.message.unwrap_or_else(|| "pak verify failed".into()),
        ));
    }
    if !result.found {
        return Err(UecmError::OperationFailed(
            ".ddp not found after generation".into(),
        ));
    }
    let size_bytes = result.size.parse().unwrap_or_default();
    Ok(PakOutput {
        path: result.path,
        size_bytes,
    })
}

pub fn launch_generation(
    backend: UeRunnerBackend,
    host: &str,
    engine_path: &str,
    project_path: &str,
    user: Option<&str>,
    pass: Option<&str>,
) -> ue_runner::RunnerHandle {
    ue_runner::run(UeRunSpec {
        backend,
        host: host.to_string(),
        engine_path: engine_path.to_string(),
        project_path: project_path.to_string(),
        extra_args: default_extra_args(),
        credential_user: user.map(String::from),
        credential_pass: pass.map(String::from),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn preflight_returns_powershell_error_on_non_windows() {
        let result = preflight("h", "C:\\UE", "C:\\X.uproject", Some("u"), Some("p"));
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn verify_returns_powershell_error_on_non_windows() {
        let result = verify_output("h", "C:\\X", Some("u"), Some("p"));
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
