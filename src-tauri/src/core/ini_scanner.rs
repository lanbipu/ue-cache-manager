//! INI scanner orchestration: enumerate target files for one machine, read
//! them via `read-ini-file.ps1`, and run the pure rule engine over the result.

use crate::core::ini_diagnostics::{
    self, Category, EnvVarState, Finding, ParsedFile, ParsedKey, ParsedSection,
};
use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct TargetFile {
    pub path: String,
    pub category: Category,
}

pub fn enumerate_engine_paths(installs: &[(String, String)]) -> Vec<TargetFile> {
    installs.iter().map(|(_, root)| TargetFile {
        path: format!("{}\\Engine\\Config\\BaseEngine.ini", root.trim_end_matches('\\')),
        category: Category::Engine,
    }).collect()
}

pub fn enumerate_user_paths(installs: &[(String, String)], user_profile: &str) -> Vec<TargetFile> {
    installs.iter().map(|(version, _)| TargetFile {
        path: format!(
            "{}\\AppData\\Local\\UnrealEngine\\{}\\Saved\\Config\\WindowsEditor\\EditorPerProjectUserSettings.ini",
            user_profile.trim_end_matches('\\'),
            version
        ),
        category: Category::User,
    }).collect()
}

pub fn enumerate_project_paths(project_roots: &[String]) -> Vec<TargetFile> {
    let mut out = Vec::new();
    for root in project_roots {
        let r = root.trim_end_matches('\\');
        out.push(TargetFile { path: format!("{}\\Config\\DefaultEngine.ini", r), category: Category::Project });
        out.push(TargetFile { path: format!("{}\\Config\\ConsoleVariables.ini", r), category: Category::Project });
        out.push(TargetFile { path: format!("{}\\Config\\Windows\\WindowsEngine.ini", r), category: Category::Project });
    }
    out
}

#[derive(Debug, Deserialize)]
struct ReadFileResult {
    pub ok: bool,
    pub found: bool,
    #[serde(default)]
    pub sections: Vec<RawSection>,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct RawSection {
    pub name: String,
    #[serde(default)]
    pub keys: Vec<RawKey>,
}

#[derive(Debug, Deserialize)]
struct RawKey {
    pub name: String,
    pub value: String,
    pub line_number: usize,
}

pub fn read_file(
    host: &str,
    target: &TargetFile,
    cred: Option<(&str, &str)>,
) -> UecmResult<Option<ParsedFile>> {
    let mut args: Vec<String> = vec![
        "-HostName".into(), host.into(),
        "-FilePath".into(), target.path.clone(),
    ];
    if let Some((u, p)) = cred {
        args.push("-Username".into()); args.push(u.into());
        args.push("-Password".into()); args.push(p.into());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: ReadFileResult = powershell::run_json(
        &powershell::script_path("read-ini-file.ps1"),
        &arg_refs,
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "read-ini-file failed: {}",
            result.message
        )));
    }
    if !result.found {
        return Ok(None);
    }
    Ok(Some(ParsedFile {
        path: target.path.clone(),
        category: target.category,
        sections: result.sections.into_iter().map(|s| ParsedSection {
            name: s.name,
            keys: s.keys.into_iter().map(|k| ParsedKey {
                name: k.name,
                value: k.value,
                line_number: k.line_number,
            }).collect(),
        }).collect(),
    }))
}

pub struct ScanInputs<'a> {
    pub host: &'a str,
    pub credential: Option<(&'a str, &'a str)>,
    pub installs: &'a [(String, String)],
    pub user_profile: &'a str,
    pub project_roots: &'a [String],
    pub env_state: EnvVarState,
}

/// Per-file outcome of one scan pass for a single machine.
///
/// `errors` keeps human-readable failure reasons (with file path attached) so the
/// caller can show "scan completed with N errors" instead of silently reporting 0
/// findings when the credential / WinRM is broken.
#[derive(Debug, Default)]
pub struct ScanOutcome {
    pub findings: Vec<Finding>,
    pub errors: Vec<String>,
}

pub fn scan_machine(inputs: &ScanInputs) -> UecmResult<ScanOutcome> {
    let mut targets: Vec<TargetFile> = Vec::new();
    targets.extend(enumerate_engine_paths(inputs.installs));
    targets.extend(enumerate_user_paths(inputs.installs, inputs.user_profile));
    targets.extend(enumerate_project_paths(inputs.project_roots));

    let mut outcome = ScanOutcome::default();
    for tf in &targets {
        match read_file(inputs.host, tf, inputs.credential) {
            Ok(Some(pf)) => outcome.findings.extend(ini_diagnostics::run_rules(&pf, &inputs.env_state)),
            Ok(None) => {}
            Err(e) => {
                let msg = format!("{}: {}", tf.path, e);
                eprintln!("[ini_scanner] {}", msg);
                outcome.errors.push(msg);
            }
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_engine_paths_returns_baseengine_per_install() {
        let installs = vec![
            ("5.4".to_string(), "C:\\Program Files\\Epic Games\\UE_5.4".to_string()),
            ("5.5".to_string(), "D:\\UE\\UE_5.5".to_string()),
        ];
        let paths = enumerate_engine_paths(&installs);
        assert_eq!(paths.len(), 2);
        assert!(paths[0].path.contains("UE_5.4"));
        assert!(paths[0].path.ends_with("Engine\\Config\\BaseEngine.ini"));
    }

    #[test]
    fn enumerate_user_paths_returns_one_per_version() {
        let installs = vec![
            ("5.4".to_string(), "C:\\anything".to_string()),
        ];
        let paths = enumerate_user_paths(&installs, "C:\\Users\\lanpc");
        assert_eq!(paths.len(), 1);
        assert!(paths[0].path.contains("AppData\\Local\\UnrealEngine\\5.4"));
        assert_eq!(paths[0].category, crate::core::ini_diagnostics::Category::User);
    }

    #[test]
    fn enumerate_project_paths_returns_three_files_per_project_path() {
        let projects = vec!["E:\\Work\\EXLY".to_string()];
        let paths = enumerate_project_paths(&projects);
        assert_eq!(paths.len(), 3);
        assert!(paths.iter().any(|p| p.path.ends_with("DefaultEngine.ini")));
        assert!(paths.iter().any(|p| p.path.ends_with("ConsoleVariables.ini")));
        assert!(paths.iter().any(|p| p.path.ends_with("WindowsEngine.ini")));
    }
}
