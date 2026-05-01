//! Discovery probes that run on a remote host via WinRM:
//! - UE installed versions (registry read)
//! - GPU model + driver version (WMI Win32_VideoController)

use crate::core::winrm;
use crate::error::UecmResult;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DetectedUe {
    pub version: String,
    pub install_path: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DetectedGpu {
    pub gpu_model: String,
    pub driver_version: String,
    pub vendor: String,
    pub vram_mb: Option<i64>,
}

pub fn detect_ue_versions(host: &str) -> UecmResult<Vec<DetectedUe>> {
    let body = read_script("query-ue-versions.ps1")?;
    let result: Vec<DetectedUe> = winrm::invoke_json(host, &body)?;
    Ok(result)
}

pub fn detect_gpus(host: &str) -> UecmResult<Vec<DetectedGpu>> {
    let body = read_script("query-gpu-driver.ps1")?;
    let result: Vec<DetectedGpu> = winrm::invoke_json(host, &body)?;
    Ok(result)
}

fn read_script(name: &str) -> UecmResult<String> {
    let path: PathBuf = Path::new("..").join("ps-scripts").join(name);
    Ok(fs::read_to_string(&path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::UecmError;

    #[test]
    fn read_script_returns_powershell_text() {
        let body = read_script("query-ue-versions.ps1").unwrap();
        assert!(body.contains("HKLM:\\SOFTWARE\\EpicGames"));
    }

    #[cfg(not(windows))]
    #[test]
    fn detect_ue_versions_returns_powershell_error_on_non_windows() {
        let result = detect_ue_versions("RENDER-01");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn detect_gpus_returns_powershell_error_on_non_windows() {
        let result = detect_gpus("RENDER-01");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
