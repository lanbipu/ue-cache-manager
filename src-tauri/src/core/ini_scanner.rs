//! INI scanner orchestration and path enumeration.

use crate::core::{env_vars, ini_diagnostics, powershell, remote_path};
use crate::data::{self, Db, IniFinding, UeInstall};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IniScanTarget {
    pub category: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IniScanSummary {
    pub scan_run_id: i64,
    pub critical: i64,
    pub warning: i64,
    pub healthy: i64,
    pub info: i64,
    pub total_files: i64,
}

#[derive(Debug, Deserialize)]
struct ReadIniFileResult {
    ok: bool,
    sections: Vec<ReadIniSection>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ReadIniSection {
    name: String,
    keys: Vec<ReadIniKey>,
}

#[derive(Debug, Deserialize)]
struct ReadIniKey {
    name: String,
    value: String,
    line_number: i64,
    raw: String,
}

pub fn enumerate_targets(
    installs: &[UeInstall],
    project_paths: &[String],
    user_profile_path: Option<&str>,
) -> Vec<IniScanTarget> {
    let mut out = Vec::new();
    for project in project_paths.iter().filter(|p| !p.trim().is_empty()) {
        let base = trim_slashes(project);
        out.push(target("project", format!("{}\\Config\\DefaultEngine.ini", base)));
        out.push(target("project", format!("{}\\Config\\ConsoleVariables.ini", base)));
        out.push(target("project", format!("{}\\Config\\Windows\\ConsoleVariables.ini", base)));
    }
    for install in installs {
        let engine = trim_slashes(&install.install_path);
        out.push(target("engine", format!("{}\\Engine\\Config\\BaseEngine.ini", engine)));
        if let Some(profile) = user_profile_path {
            out.push(target(
                "user",
                format!(
                    "{}\\AppData\\Local\\UnrealEngine\\{}\\Saved\\Config\\WindowsEditor\\EditorPerProjectUserSettings.ini",
                    trim_slashes(profile),
                    install.version
                ),
            ));
        }
    }
    out.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    out.dedup_by(|a, b| a.file_path.eq_ignore_ascii_case(&b.file_path));
    out
}

pub fn run_scan(
    db: &Db,
    machine_ids: &[i64],
    credential_alias: &str,
    project_paths: &[String],
    user_profile_path: Option<&str>,
) -> UecmResult<IniScanSummary> {
    let credential = data::credentials::find_by_alias(db, credential_alias)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("credential alias '{}' not found", credential_alias))
    })?;
    let password = crate::core::credentials::resolve_password(credential_alias)?;
    let scan_run_id = data::scan_runs::insert(db, "ini", machine_ids)?;
    let mut summary = IniScanSummary {
        scan_run_id,
        critical: 0,
        warning: 0,
        healthy: 0,
        info: 0,
        total_files: 0,
    };

    for machine_id in machine_ids {
        let machine = data::machines::find_by_id(db, *machine_id)?
            .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;
        let installs = data::machine_ue_installs::list_for_machine(db, *machine_id)?;
        let targets = enumerate_targets(&installs, project_paths, user_profile_path);
        let mut env_state = HashMap::new();
        if let Ok(value) = env_vars::get_with_credential(&machine.ip, ini_diagnostics::SHARED_DDC_ENV, &credential.username, &password) {
            env_state.insert(ini_diagnostics::SHARED_DDC_ENV.to_string(), value);
        }
        for target in targets {
            match read_file_with_credential(&machine.ip, &target.file_path, &credential.username, &password) {
                Ok(doc) => {
                    summary.total_files += 1;
                    let diagnostics_doc = to_diagnostics_doc(&target, doc);
                    let path_reachability = probe_reachable_paths(
                        &machine.ip,
                        &diagnostics_doc,
                        &env_state,
                        &credential.username,
                        &password,
                    );
                    let ctx = ini_diagnostics::DiagnosticContext {
                        env_vars: env_state.clone(),
                        path_reachability,
                    };
                    for finding in ini_diagnostics::diagnose(&diagnostics_doc, &ctx) {
                        bump_summary(&mut summary, finding.severity);
                        data::ini_findings::insert(db, &to_data_finding(scan_run_id, *machine_id, finding))?;
                    }
                }
                Err(err) => {
                    data::ini_findings::insert(db, &IniFinding {
                        id: None,
                        scan_run_id,
                        machine_id: *machine_id,
                        rule_id: "SCAN_ERROR".into(),
                        severity: "info".into(),
                        category: target.category,
                        file_path: target.file_path,
                        section: None,
                        key_name: None,
                        line_number: None,
                        snippet_before: "".into(),
                        snippet_after: None,
                        recommended_action: "manual".into(),
                        recommended_value: None,
                        symptom: "INI file could not be read.".into(),
                        rationale: err.to_string(),
                        fixed_at: None,
                        skipped_at: None,
                    })?;
                    summary.info += 1;
                }
            }
        }
    }

    data::scan_runs::finish(
        db,
        scan_run_id,
        &serde_json::json!({
            "critical": summary.critical,
            "warning": summary.warning,
            "healthy": summary.healthy,
            "info": summary.info,
            "total_files": summary.total_files
        }),
    )?;
    Ok(summary)
}

fn read_file_with_credential(
    host: &str,
    file_path: &str,
    username: &str,
    password: &str,
) -> UecmResult<ReadIniFileResult> {
    let result: ReadIniFileResult = powershell::run_json(
        &powershell::script_path("read-ini-file.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(result.message));
    }
    Ok(result)
}

fn to_diagnostics_doc(target: &IniScanTarget, read: ReadIniFileResult) -> ini_diagnostics::IniDocument {
    let entries = read
        .sections
        .into_iter()
        .flat_map(|section| {
            section.keys.into_iter().map(move |key| ini_diagnostics::IniEntry {
                section: section.name.clone(),
                key: key.name,
                value: key.value,
                line_number: key.line_number,
                raw: key.raw,
            })
        })
        .collect();
    ini_diagnostics::IniDocument {
        file_path: target.file_path.clone(),
        category: target.category.clone(),
        entries,
    }
}

fn to_data_finding(scan_run_id: i64, machine_id: i64, finding: ini_diagnostics::Finding) -> IniFinding {
    IniFinding {
        id: None,
        scan_run_id,
        machine_id,
        rule_id: finding.rule_id,
        severity: finding.severity.as_str().into(),
        category: finding.category,
        file_path: finding.file_path,
        section: finding.section,
        key_name: finding.key_name,
        line_number: finding.line_number,
        snippet_before: finding.snippet_before,
        snippet_after: finding.snippet_after,
        recommended_action: finding.recommended_action,
        recommended_value: finding.recommended_value,
        symptom: finding.symptom,
        rationale: finding.rationale,
        fixed_at: None,
        skipped_at: None,
    }
}

fn probe_reachable_paths(
    host: &str,
    doc: &ini_diagnostics::IniDocument,
    env_state: &HashMap<String, Option<String>>,
    username: &str,
    password: &str,
) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    for path in collect_reachable_candidates(doc, env_state) {
        let reachable = remote_path::exists_with_credential(host, &path, username, password).unwrap_or(false);
        out.insert(path, reachable);
    }
    out
}

fn collect_reachable_candidates(
    doc: &ini_diagnostics::IniDocument,
    env_state: &HashMap<String, Option<String>>,
) -> Vec<String> {
    let mut out = Vec::new();
    for entry in &doc.entries {
        if !entry.section.eq_ignore_ascii_case(ini_diagnostics::DDC_SECTION) {
            continue;
        }
        let key = ini_diagnostics::normalized_key(&entry.key);
        if key == "Path" && looks_like_path(&entry.value) {
            out.push(entry.value.clone());
        } else if key == "EnvPathOverride" {
            if let Some(Some(value)) = env_state.get(entry.value.trim()) {
                if looks_like_path(value) {
                    out.push(value.clone());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn looks_like_path(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("\\\\")
        || value.starts_with("//")
        || (value.len() >= 3 && value.as_bytes()[1] == b':' && value.as_bytes()[0].is_ascii_alphabetic())
}

fn bump_summary(summary: &mut IniScanSummary, severity: ini_diagnostics::Severity) {
    match severity {
        ini_diagnostics::Severity::Critical => summary.critical += 1,
        ini_diagnostics::Severity::Warning => summary.warning += 1,
        ini_diagnostics::Severity::Healthy => summary.healthy += 1,
        ini_diagnostics::Severity::Info => summary.info += 1,
    }
}

fn target(category: &str, file_path: String) -> IniScanTarget {
    IniScanTarget {
        category: category.into(),
        file_path,
    }
}

fn trim_slashes(value: &str) -> &str {
    value.trim().trim_end_matches(['\\', '/'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_targets_includes_project_engine_and_user_files() {
        let installs = vec![UeInstall {
            id: None,
            machine_id: 1,
            version: "5.4".into(),
            install_path: "C:\\UE_5.4".into(),
            is_primary: true,
        }];
        let targets = enumerate_targets(&installs, &["E:\\Proj".into()], Some("C:\\Users\\lanpc"));
        assert!(targets.iter().any(|t| t.file_path.ends_with("DefaultEngine.ini")));
        assert!(targets.iter().any(|t| t.file_path.ends_with("BaseEngine.ini")));
        assert!(targets.iter().any(|t| t.file_path.ends_with("EditorPerProjectUserSettings.ini")));
    }

    #[test]
    fn collect_reachable_candidates_includes_ddc_path_and_env_value() {
        let doc = ini_diagnostics::parse_ini(
            "DefaultEngine.ini",
            "project",
            "[/Script/UnrealEd.DerivedDataCacheSettings]\nPath=\\\\HOST\\Bad\nEnvPathOverride=UE-SharedDataCachePath",
        );
        let mut env = HashMap::new();
        env.insert(
            ini_diagnostics::SHARED_DDC_ENV.into(),
            Some("\\\\HOST\\Good".into()),
        );
        let paths = collect_reachable_candidates(&doc, &env);
        assert_eq!(paths, vec!["\\\\HOST\\Bad", "\\\\HOST\\Good"]);
    }
}
