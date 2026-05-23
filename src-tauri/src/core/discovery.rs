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
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

/// SSH 连不上时给可操作提示：节点很可能没经 `UECM-Bootstrap.cmd` 做 SSH 纳管。
/// SSH 纳管唯一路径是节点本地双击 bootstrap（operator 远程纳管已按 spec 退役）。
fn with_onboarding_hint(host: &str, e: UecmError) -> UecmError {
    match e {
        UecmError::SshConnect(m) => UecmError::SshConnect(format!(
            "{m} -- host {host} may not be SSH-onboarded; run UECM-Bootstrap.cmd on it"
        )),
        other => other,
    }
}

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
    .map_err(|e| with_onboarding_hint(host, e))
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
    .map_err(|e| with_onboarding_hint(host, e))
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

    struct FailExec;
    impl RemoteExecutor for FailExec {
        fn run(&self, _h: &str, _s: &NodeScript) -> UecmResult<ScriptOutput> {
            // exit 255 -> run_json maps to SshConnect
            Ok(ScriptOutput {
                stdout: String::new(),
                stderr: "Connection refused".into(),
                exit_code: 255,
            })
        }
        fn probe(&self, _h: &str, _u: Option<&str>) -> UecmResult<ProbeResult> {
            unreachable!()
        }
    }

    #[test]
    fn detect_adds_onboarding_hint_on_ssh_connect_failure() {
        let err = detect_ue_versions(&FailExec, "RENDER-01").unwrap_err();
        assert!(
            err.to_string().contains("UECM-Bootstrap.cmd"),
            "expected onboarding hint, got: {err}"
        );
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
