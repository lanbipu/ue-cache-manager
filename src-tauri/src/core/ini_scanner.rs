//! INI scanner orchestration: enumerate target files for one machine, read
//! them via `read-ini-file.ps1`, and run the pure rule engine over the result.

use crate::core::ini_diagnostics::{
    self, Category, EnvVarState, Finding, ParsedFile, ParsedKey, ParsedSection,
};
use crate::core::{loopback, powershell};
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;
use std::io::ErrorKind;

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
    if loopback::is_loopback_target(host) {
        let _ = cred;
        return read_local_file(target);
    }

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

fn read_local_file(target: &TargetFile) -> UecmResult<Option<ParsedFile>> {
    let contents = match std::fs::read_to_string(&target.path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(UecmError::OperationFailed(format!(
                "read local INI failed: {}",
                e
            )));
        }
    };
    Ok(Some(parse_ini_contents(target, &contents)))
}

fn parse_ini_contents(target: &TargetFile, contents: &str) -> ParsedFile {
    let mut sections = Vec::new();
    let mut current: Option<ParsedSection> = None;

    for (idx, line) in contents.lines().enumerate() {
        let line_number = idx + 1;
        let trim = line.trim();
        if trim.starts_with('[') && trim.ends_with(']') && trim.len() > 2 {
            if let Some(section) = current.take() {
                sections.push(section);
            }
            current = Some(ParsedSection {
                name: trim[1..trim.len() - 1].to_string(),
                keys: Vec::new(),
            });
            continue;
        }

        let Some(section) = current.as_mut() else {
            continue;
        };
        if trim.is_empty()
            || trim.starts_with(';')
            || trim.starts_with('#')
            || trim.starts_with("//")
        {
            continue;
        }
        if let Some(eq) = trim.find('=') {
            if eq > 0 {
                section.keys.push(ParsedKey {
                    name: trim[..eq].trim().to_string(),
                    value: trim[eq + 1..].trim().to_string(),
                    line_number,
                });
            }
        }
    }

    if let Some(section) = current {
        sections.push(section);
    }

    ParsedFile {
        path: target.path.clone(),
        category: target.category,
        sections,
    }
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

    #[test]
    fn read_file_uses_local_filesystem_for_loopback_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DefaultEngine.ini");
        std::fs::write(
            &path,
            "[/Script/Engine.RendererSettings]\nr.PSOPrecaching=1\n",
        )
        .unwrap();

        let target = TargetFile {
            path: path.to_string_lossy().to_string(),
            category: Category::Project,
        };
        let parsed = read_file("localhost", &target, Some(("ignored", "ignored")))
            .unwrap()
            .unwrap();

        assert_eq!(parsed.sections[0].name, "/Script/Engine.RendererSettings");
        assert_eq!(parsed.sections[0].keys[0].name, "r.PSOPrecaching");
        assert_eq!(parsed.sections[0].keys[0].value, "1");
    }
}
