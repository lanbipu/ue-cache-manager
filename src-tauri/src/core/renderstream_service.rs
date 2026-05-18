//! Probes RenderStream-related Windows services on a host. Classifies risky
//! account configurations (LocalSystem / local-interactive-user). Provides
//! `into_check_outcome` for embedding in the health round-trip.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceFact {
    #[serde(rename = "Name")] pub name: String,
    #[serde(rename = "DisplayName")] pub display_name: String,
    #[serde(rename = "StartName")] pub start_name: String,
    #[serde(rename = "State")] pub state: String,
    #[serde(rename = "StartMode")] pub start_mode: String,
    #[serde(rename = "PathName")] pub path_name: String,
}

#[derive(Debug, Deserialize)]
struct ScriptResult {
    ok: bool,
    services: Option<Vec<ServiceFact>>,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RsServiceReport {
    pub host: String,
    pub services: Vec<ServiceFact>,
    pub risks: Vec<String>,
}

pub fn classify_risks(services: &[ServiceFact]) -> Vec<String> {
    let mut out = Vec::new();
    for s in services {
        let acct = s.start_name.to_lowercase();
        let is_local_system = acct == "localsystem" || acct == ".\\localsystem";
        let is_network_service = acct.contains("networkservice");
        let looks_local_user = acct.starts_with(".\\")
            || (!acct.contains('@') && !acct.contains('\\') && !is_local_system && !is_network_service);

        if is_local_system {
            out.push(format!(
                "{} runs as LocalSystem — user-level env vars and Editor Preferences are invisible. DDC paths must be in Machine-scope env or Project Config.",
                s.name
            ));
        } else if looks_local_user {
            out.push(format!(
                "{} runs as a local interactive user ({}) — Editor Preferences set under a different account will not apply.",
                s.name, s.start_name
            ));
        }
        if s.state != "Running" {
            out.push(format!("{} is not running ({})", s.name, s.state));
        }
    }
    out
}

pub fn report(host: &str, creds: Option<(&str, &str)>) -> UecmResult<RsServiceReport> {
    let mut args: Vec<&str> = vec!["-HostName", host];
    if let Some((u, p)) = creds {
        args.extend(["-Username", u, "-Password", p]);
    }
    let r: ScriptResult = powershell::run_json(
        &powershell::script_path("probe-renderstream-service.ps1"),
        &args,
    )?;
    if !r.ok {
        return Err(UecmError::OperationFailed(
            r.message.unwrap_or_else(|| "probe failed".into()),
        ));
    }
    let services = r.services.unwrap_or_default();
    let risks = classify_risks(&services);
    Ok(RsServiceReport { host: host.to_string(), services, risks })
}

/// Reduce a detailed report to a single CheckOutcome shape suitable for
/// inclusion in the flat health round-trip HashMap. The detailed services
/// array is exposed separately via a Tauri command (M4.3 wires it).
pub fn into_check_outcome(report: &RsServiceReport) -> crate::core::health_check::CheckOutcome {
    let status = if !report.risks.is_empty() {
        "warning"
    } else if report.services.is_empty() {
        "na"
    } else {
        "healthy"
    };
    let names: Vec<&str> = report.services.iter().map(|s| s.name.as_str()).collect();
    let message = if report.services.is_empty() {
        "no RenderStream service detected".to_string()
    } else if !report.risks.is_empty() {
        report.risks.join("; ")
    } else {
        format!("services: {}", names.join(", "))
    };
    crate::core::health_check::CheckOutcome {
        status: status.into(),
        message,
        sample: names.join(", "),
        remediation: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(name: &str, start_name: &str, state: &str) -> ServiceFact {
        ServiceFact {
            name: name.into(),
            display_name: name.into(),
            start_name: start_name.into(),
            state: state.into(),
            start_mode: "Auto".into(),
            path_name: r"C:\d3\bin\foo.exe".into(),
        }
    }

    #[test]
    fn local_system_account_flagged() {
        let r = classify_risks(&[svc("d3service", "LocalSystem", "Running")]);
        assert!(r.iter().any(|x| x.contains("LocalSystem")));
    }
    #[test]
    fn local_interactive_user_flagged() {
        let r = classify_risks(&[svc("d3service", ".\\disguise", "Running")]);
        assert!(r.iter().any(|x| x.contains("local interactive")));
    }
    #[test]
    fn non_running_flagged() {
        let r = classify_risks(&[svc("d3service", "LocalSystem", "Stopped")]);
        assert!(r.iter().any(|x| x.contains("not running")));
    }
    #[test]
    fn no_risk_for_domain_account() {
        let r = classify_risks(&[svc("d3service", "domain\\svc", "Running")]);
        assert!(r.is_empty());
    }

    #[test]
    fn into_check_outcome_na_when_empty() {
        let report = RsServiceReport { host: "X".into(), services: vec![], risks: vec![] };
        let outcome = into_check_outcome(&report);
        assert_eq!(outcome.status, "na");
    }

    #[test]
    fn into_check_outcome_warning_with_risks() {
        let report = RsServiceReport {
            host: "X".into(),
            services: vec![svc("d3service", "LocalSystem", "Running")],
            risks: vec!["d3service runs as LocalSystem".into()],
        };
        let outcome = into_check_outcome(&report);
        assert_eq!(outcome.status, "warning");
    }
}
