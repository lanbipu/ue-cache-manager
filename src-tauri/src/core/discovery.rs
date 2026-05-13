//! Discovery probes that run on a remote host via WinRM:
//! - UE installed versions (registry read)
//! - GPU model + driver version (WMI Win32_VideoController)

use crate::core::{powershell, winrm};
use crate::data::GpuVendor;
use crate::error::UecmResult;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DetectedUe {
    pub version: String,
    pub install_path: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DetectedGpu {
    pub gpu_model: String,
    pub driver_version: String,
    pub vendor: GpuVendor,
    pub vram_mb: Option<i64>,
}

pub fn detect_ue_versions(host: &str) -> UecmResult<Vec<DetectedUe>> {
    let body = powershell::read_script("query-ue-versions.ps1")?;
    winrm::invoke_json(host, &body)
}

pub fn detect_gpus(host: &str) -> UecmResult<Vec<DetectedGpu>> {
    let body = powershell::read_script("query-gpu-driver.ps1")?;
    winrm::invoke_json(host, &body)
}

#[cfg(test)]
mod tests {
    use crate::core::powershell;
    #[cfg(not(windows))]
    use crate::error::UecmError;
    #[cfg(not(windows))]
    use super::{detect_gpus, detect_ue_versions};

    #[test]
    fn discovery_scripts_are_loadable() {
        let body = powershell::read_script("query-ue-versions.ps1").unwrap();
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
