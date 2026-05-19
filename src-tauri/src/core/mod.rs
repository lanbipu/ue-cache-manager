pub mod batch;
pub mod bootstrap;
pub mod command_line_scanner;
pub mod consistency_check;
pub mod credentials;
pub mod ddc_file_stats;
pub mod ddc_pak;
pub mod ddc_symptom_recognizer;
pub mod deploy_workflow;
pub mod discovery;
pub mod editor_preferences;
pub mod env_vars;
pub mod gpu_consistency;
pub mod health_check;
pub mod health_probes;
pub mod ini_apply;
pub mod ini_backend_graph;
pub mod ini_diagnostics;
pub mod ini_editor;
pub mod ini_scanner;
pub mod local_cache;
pub mod loopback;
pub mod network;
pub mod pak_distribute;
pub mod powershell;
pub mod preflight;
pub mod probe_keys;
pub mod project_discovery;
pub mod project_identity;
pub mod pso_collect;
pub mod pso_distribute;
pub mod psexec;
pub mod renderstream_service;
pub mod shares;
pub mod ue_log_parser;
pub mod ue_log_verify;
pub mod ue_runner;
pub mod winrm;
pub mod zen;

#[cfg(test)]
mod bootstrap_contract_tests {
    use super::*;

    #[test]
    fn manual_winrm_bootstrap_script_contains_required_commands() {
        let script = bootstrap::manual_winrm_script();
        assert!(script.contains("Enable-PSRemoting -Force"));
        assert!(script.contains("winrm quickconfig -q"));
        assert!(script.contains("Test-WSMan localhost"));
    }

    #[test]
    fn remote_bootstrap_updates_operator_trustedhosts() {
        let script = include_str!("../../../ps-scripts/bootstrap-winrm-remote.ps1");
        assert!(script.contains("WSMan:\\localhost\\Client\\TrustedHosts"));
        assert!(script.contains("Set-Item"));
        assert!(script.contains("operator TrustedHosts"));
    }

    #[test]
    fn manual_winrm_checkonly_reports_actual_wsman_state() {
        let script = bootstrap::manual_winrm_script();
        assert!(script.contains("$checkState = Get-UecmWinRmState"));
        assert!(script.contains("ok = $checkState.wsman_localhost_ok"));
    }

    #[test]
    fn remote_bootstrap_uses_target_systemroot() {
        let script = include_str!("../../../ps-scripts/bootstrap-winrm-remote.ps1");
        assert!(script.contains("%SystemRoot%\\Temp"));
        assert!(!script.contains("C:\\Windows\\Temp"));
    }

    #[cfg(not(windows))]
    #[test]
    fn psexec_bootstrap_returns_powershell_error_on_non_windows() {
        let result =
            bootstrap::enable_winrm_with_psexec("192.168.10.173", "admin", "secret", false);
        assert!(matches!(result, Err(crate::error::UecmError::PowerShell(_))));
    }
}
