//! Discovery probes that run a node-pure PowerShell script on a remote host via SSH:
//! - UE installed versions (registry read)    -> query-ue-versions.ps1
//! - GPU model + driver version (WMI)          -> query-gpu-driver.ps1
//!
//! The scripts are staged on the node (enable-ssh.ps1 / scp_push) and run via
//! `-File`. They take no args, so this module just ships an empty arg object and
//! parses the JSON the script emits to stdout. Auth is the SSH key (no per-call
//! credentials), so the old WinRM `*_with_credential` variants are gone.

use crate::core::ssh::{run_json, NodeScript, RemoteExecutor};
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

pub fn detect_ue_versions(exec: &dyn RemoteExecutor, host: &str) -> UecmResult<Vec<DetectedUe>> {
    run_json(
        exec,
        host,
        &NodeScript {
            name: "query-ue-versions.ps1",
            args: serde_json::json!({}),
            ssh_user: None,
        },
    )
}

pub fn detect_gpus(exec: &dyn RemoteExecutor, host: &str) -> UecmResult<Vec<DetectedGpu>> {
    run_json(
        exec,
        host,
        &NodeScript {
            name: "query-gpu-driver.ps1",
            args: serde_json::json!({}),
            ssh_user: None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::powershell;
    use crate::core::ssh::{ProbeResult, ScriptOutput};
    use crate::ENV_TEST_LOCK;

    // Scripts must remain on disk (they get staged to the node). This guards
    // against an accidental rename/delete breaking discovery. Holds the crate
    // env lock because the path resolver reads UECM_PS_DIR.
    #[test]
    fn discovery_scripts_are_loadable() {
        let _lock = ENV_TEST_LOCK.lock().unwrap();
        let body = powershell::read_script("query-ue-versions.ps1").unwrap();
        assert!(body.contains("HKLM:\\SOFTWARE\\EpicGames"));
        let gpu = powershell::read_script("query-gpu-driver.ps1").unwrap();
        assert!(!gpu.trim().is_empty());
    }

    struct FakeExec(String);
    impl RemoteExecutor for FakeExec {
        fn run(&self, _h: &str, _s: &NodeScript) -> UecmResult<ScriptOutput> {
            Ok(ScriptOutput {
                stdout: self.0.clone(),
                stderr: String::new(),
                exit_code: 0,
            })
        }
        fn probe(&self, _h: &str, _u: Option<&str>) -> UecmResult<ProbeResult> {
            Ok(ProbeResult {
                ok: true,
                message: "fake".into(),
                latency_ms: 1,
            })
        }
    }

    #[test]
    fn detect_ue_versions_parses_node_json() {
        let exec = FakeExec(r#"[{"version":"5.4","install_path":"C:\\UE_5.4"}]"#.to_string());
        let v = detect_ue_versions(&exec, "RENDER-01").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].version, "5.4");
        assert_eq!(v[0].install_path, "C:\\UE_5.4");
    }

    #[test]
    fn detect_gpus_parses_node_json() {
        let exec = FakeExec(
            r#"[{"gpu_model":"RTX 4090","driver_version":"551.86","vendor":"nvidia","vram_mb":24576}]"#
                .to_string(),
        );
        let g = detect_gpus(&exec, "RENDER-01").unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].gpu_model, "RTX 4090");
    }

    /// Real-node integration (default ignore). Needs query-ue-versions.ps1 +
    /// query-gpu-driver.ps1 staged on the node. Run:
    /// `UECM_IT_HOST=.. UECM_IT_USER=.. UECM_IT_KEY=.. UECM_IT_KNOWN_HOSTS=.. \`
    /// `cargo test --lib core::discovery::tests::it_detect_against_real_node -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn it_detect_against_real_node() {
        use crate::core::ssh::{SshExecutor, STAGING_ROOT};
        let (host, user, key, kh) = match (
            std::env::var("UECM_IT_HOST"),
            std::env::var("UECM_IT_USER"),
            std::env::var("UECM_IT_KEY"),
            std::env::var("UECM_IT_KNOWN_HOSTS"),
        ) {
            (Ok(h), Ok(u), Ok(k), Ok(kh)) => (h, u, k, kh),
            _ => {
                eprintln!("skip: set UECM_IT_HOST/USER/KEY/KNOWN_HOSTS");
                return;
            }
        };
        let exec = SshExecutor {
            key_path: std::path::PathBuf::from(key),
            known_hosts: std::path::PathBuf::from(kh),
            default_user: user,
            staging_root: STAGING_ROOT.to_string(),
        };
        let ue = detect_ue_versions(&exec, &host).unwrap();
        eprintln!("detected {} UE install(s)", ue.len());
        let gpus = detect_gpus(&exec, &host).unwrap();
        eprintln!("detected {} GPU(s)", gpus.len());
        assert!(!gpus.is_empty(), "a real Windows node reports >=1 video controller");
    }
}
