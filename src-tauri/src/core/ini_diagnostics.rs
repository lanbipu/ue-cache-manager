//! Pure rule engine. Takes a parsed INI file + env-var state, emits findings.
//! No Windows-specific calls; runs and tests on every platform.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Healthy,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::Warning => "warning",
            Severity::Healthy => "healthy",
            Severity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Project,
    User,
    Engine,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Project => "project",
            Category::User => "user",
            Category::Engine => "engine",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedKey {
    pub name: String,
    pub value: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedSection {
    pub name: String,
    pub keys: Vec<ParsedKey>,
    pub backend_nodes: Vec<crate::core::ini_backend_graph::BackendNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFile {
    pub path: String,
    pub category: Category,
    pub sections: Vec<ParsedSection>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnvVarState {
    pub shared_data_cache_path: Option<String>,
    pub local_data_cache_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub rule_id: String,
    pub severity: Severity,
    pub category: Category,
    pub file_path: String,
    pub section: Option<String>,
    pub key_name: Option<String>,
    pub line_number: Option<i64>,
    pub snippet_before: String,
    pub snippet_after: Option<String>,
    pub recommended_action: RecommendedAction,
    pub recommended_value: Option<String>,
    pub symptom: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendedAction {
    Set,
    Remove,
    Manual,
}

impl RecommendedAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecommendedAction::Set => "set",
            RecommendedAction::Remove => "remove",
            RecommendedAction::Manual => "manual",
        }
    }
}

const DDC_SECTION: &str = "/Script/UnrealEd.DerivedDataCacheSettings";

pub const DEPRECATED_CVARS: &[&str] = &[
    "r.SShaderCache",
    "r.ShaderCache",
    "s.SkipFinalizeCommandList",
    "r.UseShaderCaching",
];

pub fn run_rules(file: &ParsedFile, env: &EnvVarState) -> Vec<Finding> {
    let mut out = Vec::new();
    out.extend(rule_r001(file));
    out.extend(rule_r002(file));
    out.extend(rule_r004(file));
    out.extend(rule_r005(file));
    out.extend(rule_r006(file, env));
    out.extend(rule_r007(file, env));
    out.extend(pso_cvar_rule(
        file,
        "R008",
        "r.PSOPrecaching",
        Severity::Critical,
        "PSO precaching is disabled or not configured.",
        "Runtime PSO precaching must be enabled before collecting and distributing useful PSO cache files.",
    ));
    out.extend(pso_cvar_rule(
        file,
        "R009",
        "r.PSOPrecache.Compile",
        Severity::Warning,
        "PSO precache compilation is disabled or not configured.",
        "UE versions and project configs can leave compile behavior disabled unless explicitly set.",
    ));
    out.extend(pso_cvar_rule(
        file,
        "R010",
        "r.PSOPrecache.GlobalShaders",
        Severity::Warning,
        "Global shader PSO precaching is disabled or not configured.",
        "Global shader precaching helps keep runtime PSO cache behavior consistent across the cluster.",
    ));
    out
}

fn find_ddc(file: &ParsedFile) -> Option<&ParsedSection> {
    file.sections.iter().find(|s| s.name == DDC_SECTION)
}

fn key<'a>(section: &'a ParsedSection, name: &str) -> Option<&'a ParsedKey> {
    section.keys.iter().find(|k| k.name.eq_ignore_ascii_case(name))
}

fn rule_r001(file: &ParsedFile) -> Vec<Finding> {
    let Some(section) = find_ddc(file) else { return vec![]; };
    let path_key = key(section, "Path");
    let env_override = key(section, "EnvPathOverride");
    if path_key.is_some() && env_override.is_none() {
        let pk = path_key.unwrap();
        return vec![Finding {
            rule_id: "R001".into(),
            severity: Severity::Critical,
            category: file.category,
            file_path: file.path.clone(),
            section: Some(section.name.clone()),
            key_name: Some(pk.name.clone()),
            line_number: Some(pk.line_number as i64),
            snippet_before: format!("Path={}", pk.value),
            snippet_after: Some("EnvPathOverride=UE-SharedDataCachePath".into()),
            recommended_action: RecommendedAction::Set,
            recommended_value: Some("UE-SharedDataCachePath".into()),
            symptom: "DDC silently uses the hardcoded path; env-var overrides are ignored.".into(),
            rationale: "When `Path=` is set without `EnvPathOverride`, UE skips the env-var lookup. The cluster cannot share DDC.".into(),
        }];
    }
    vec![]
}

fn rule_r002(file: &ParsedFile) -> Vec<Finding> {
    if file.category != Category::User { return vec![]; }
    let Some(section) = find_ddc(file) else { return vec![]; };
    if section.keys.is_empty() { return vec![]; }
    vec![Finding {
        rule_id: "R002".into(),
        severity: Severity::Critical,
        category: file.category,
        file_path: file.path.clone(),
        section: Some(section.name.clone()),
        key_name: None,
        line_number: section.keys.first().map(|k| k.line_number as i64),
        snippet_before: section.keys.iter()
            .map(|k| format!("{}={}", k.name, k.value))
            .collect::<Vec<_>>()
            .join("\n"),
        snippet_after: Some("(remove the entire DDC section from this user-level file)".into()),
        recommended_action: RecommendedAction::Remove,
        recommended_value: None,
        symptom: "User-level DDC override silently overrides project + env-var configs.".into(),
        rationale: "EditorPerProjectUserSettings.ini is the highest-priority DDC source. Any DDC keys here will mask the cluster setup.".into(),
    }]
}

fn rule_r004(file: &ParsedFile) -> Vec<Finding> {
    let Some(section) = find_ddc(file) else { return vec![]; };
    let mut out = Vec::new();
    for k in &section.keys {
        if !k.name.eq_ignore_ascii_case("Path") { continue; }
        let v = k.value.trim();
        let starts_with_drive = v.len() >= 2
            && v.chars().nth(1) == Some(':')
            && v.chars().next().map_or(false, |c| c.is_ascii_alphabetic());
        let is_unc = v.starts_with("\\\\");
        if starts_with_drive && !is_unc {
            out.push(Finding {
                rule_id: "R004".into(),
                severity: Severity::Warning,
                category: file.category,
                file_path: file.path.clone(),
                section: Some(section.name.clone()),
                key_name: Some(k.name.clone()),
                line_number: Some(k.line_number as i64),
                snippet_before: format!("Path={}", v),
                snippet_after: Some("Path=\\\\HOST\\Share\\...".into()),
                recommended_action: RecommendedAction::Manual,
                recommended_value: None,
                symptom: "Mapped drive letters are not visible to Windows Services (e.g. RenderStream).".into(),
                rationale: "Use UNC paths so SYSTEM-context processes can resolve the share.".into(),
            });
        }
    }
    out
}

fn rule_r005(file: &ParsedFile) -> Vec<Finding> {
    let mut out = Vec::new();
    for s in &file.sections {
        for k in &s.keys {
            if DEPRECATED_CVARS.iter().any(|d| d.eq_ignore_ascii_case(&k.name)) {
                out.push(Finding {
                    rule_id: "R005".into(),
                    severity: Severity::Warning,
                    category: file.category,
                    file_path: file.path.clone(),
                    section: Some(s.name.clone()),
                    key_name: Some(k.name.clone()),
                    line_number: Some(k.line_number as i64),
                    snippet_before: format!("{}={}", k.name, k.value),
                    snippet_after: Some("(remove this line)".into()),
                    recommended_action: RecommendedAction::Remove,
                    recommended_value: None,
                    symptom: "Deprecated CVar that no longer functions in UE 5.x.".into(),
                    rationale: format!("`{}` was removed; keeping it adds confusion at no benefit.", k.name),
                });
            }
        }
    }
    out
}

fn rule_r006(file: &ParsedFile, env: &EnvVarState) -> Vec<Finding> {
    let Some(section) = find_ddc(file) else { return vec![]; };
    let Some(envk) = key(section, "EnvPathOverride") else { return vec![]; };
    let v = envk.value.trim();
    let referenced_present = match v {
        "UE-SharedDataCachePath" => env.shared_data_cache_path.as_ref().is_some(),
        "UE-LocalDataCachePath" => env.local_data_cache_path.as_ref().is_some(),
        _ => true,
    };
    if !referenced_present {
        return vec![Finding {
            rule_id: "R006".into(),
            severity: Severity::Warning,
            category: file.category,
            file_path: file.path.clone(),
            section: Some(section.name.clone()),
            key_name: Some(envk.name.clone()),
            line_number: Some(envk.line_number as i64),
            snippet_before: format!("EnvPathOverride={}", v),
            snippet_after: Some(format!("(set environment variable `{}` on this machine)", v)),
            recommended_action: RecommendedAction::Manual,
            recommended_value: None,
            symptom: "INI references an env var that is not set; DDC falls back to local.".into(),
            rationale: format!("`{}` is not present on this machine. Use UECM env-var modal to set it.", v),
        }];
    }
    vec![]
}

fn rule_r007(file: &ParsedFile, env: &EnvVarState) -> Vec<Finding> {
    let Some(section) = find_ddc(file) else { return vec![]; };
    let Some(envk) = key(section, "EnvPathOverride") else { return vec![]; };
    let referenced_present = match envk.value.trim() {
        "UE-SharedDataCachePath" => env.shared_data_cache_path.is_some(),
        "UE-LocalDataCachePath" => env.local_data_cache_path.is_some(),
        _ => false,
    };
    if !referenced_present { return vec![]; }
    vec![Finding {
        rule_id: "R007".into(),
        severity: Severity::Healthy,
        category: file.category,
        file_path: file.path.clone(),
        section: Some(section.name.clone()),
        key_name: Some(envk.name.clone()),
        line_number: Some(envk.line_number as i64),
        snippet_before: format!("EnvPathOverride={}", envk.value),
        snippet_after: None,
        recommended_action: RecommendedAction::Manual,
        recommended_value: None,
        symptom: "Configured correctly. Tracked for healthy-count summary.".into(),
        rationale: "EnvPathOverride references a populated env var on this machine.".into(),
    }]
}

fn pso_cvar_rule(
    file: &ParsedFile,
    rule_id: &str,
    key_name: &str,
    severity: Severity,
    symptom: &str,
    rationale: &str,
) -> Vec<Finding> {
    if !file
        .path
        .to_ascii_lowercase()
        .ends_with("consolevariables.ini")
    {
        return vec![];
    }

    let Some(section) = file
        .sections
        .iter()
        .find(|section| section.name.eq_ignore_ascii_case("ConsoleVariables"))
    else {
        return vec![pso_missing_finding(
            file, rule_id, key_name, severity, symptom, rationale, None,
        )];
    };

    match key(section, key_name) {
        Some(entry) if entry.value.trim() == "1" => vec![],
        Some(entry) => vec![Finding {
            rule_id: rule_id.into(),
            severity,
            category: file.category,
            file_path: file.path.clone(),
            section: Some(section.name.clone()),
            key_name: Some(entry.name.clone()),
            line_number: Some(entry.line_number as i64),
            snippet_before: format!("{}={}", entry.name, entry.value),
            snippet_after: Some(format!("{}=1", key_name)),
            recommended_action: RecommendedAction::Set,
            recommended_value: Some("1".into()),
            symptom: symptom.into(),
            rationale: rationale.into(),
        }],
        None => vec![pso_missing_finding(
            file,
            rule_id,
            key_name,
            severity,
            symptom,
            rationale,
            Some(section.name.clone()),
        )],
    }
}

fn pso_missing_finding(
    file: &ParsedFile,
    rule_id: &str,
    key_name: &str,
    severity: Severity,
    symptom: &str,
    rationale: &str,
    section: Option<String>,
) -> Finding {
    Finding {
        rule_id: rule_id.into(),
        severity,
        category: file.category,
        file_path: file.path.clone(),
        section: section.or_else(|| Some("ConsoleVariables".into())),
        key_name: Some(key_name.into()),
        line_number: None,
        snippet_before: "(missing)".into(),
        snippet_after: Some(format!("{}=1", key_name)),
        recommended_action: RecommendedAction::Set,
        recommended_value: Some("1".into()),
        symptom: symptom.into(),
        rationale: rationale.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ddc_section(keys: &[(&str, &str)]) -> ParsedSection {
        ParsedSection {
            name: "/Script/UnrealEd.DerivedDataCacheSettings".into(),
            keys: keys.iter().map(|(k, v)| ParsedKey {
                name: k.to_string(),
                value: v.to_string(),
                line_number: 0,
            }).collect(),
            backend_nodes: vec![],
        }
    }

    #[test]
    fn r001_critical_when_path_set_without_envpathoverride() {
        let file = ParsedFile {
            path: "C:\\Project\\Config\\DefaultEngine.ini".into(),
            category: Category::Project,
            sections: vec![ddc_section(&[("Path", "D:\\OldDDC")])],
        };
        let env_state = EnvVarState::default();
        let findings = run_rules(&file, &env_state);
        assert!(findings.iter().any(|f| f.rule_id == "R001" && f.severity == Severity::Critical));
    }

    #[test]
    fn r001_healthy_when_envpathoverride_set_and_envvar_present() {
        let file = ParsedFile {
            path: "C:\\Project\\Config\\DefaultEngine.ini".into(),
            category: Category::Project,
            sections: vec![ddc_section(&[("EnvPathOverride", "UE-SharedDataCachePath")])],
        };
        let mut env_state = EnvVarState::default();
        env_state.shared_data_cache_path = Some("\\\\HOST\\DDC".into());
        let findings = run_rules(&file, &env_state);
        assert!(findings.iter().any(|f| f.rule_id == "R007" && f.severity == Severity::Healthy));
    }

    #[test]
    fn r002_critical_when_user_level_file_has_ddc_section() {
        let file = ParsedFile {
            path: "C:\\Users\\X\\AppData\\Local\\UnrealEngine\\5.4\\Saved\\Config\\WindowsEditor\\EditorPerProjectUserSettings.ini".into(),
            category: Category::User,
            sections: vec![ddc_section(&[("Path", "C:\\local")])],
        };
        let findings = run_rules(&file, &EnvVarState::default());
        assert!(findings.iter().any(|f| f.rule_id == "R002" && f.severity == Severity::Critical));
    }

    #[test]
    fn r004_warning_when_path_uses_drive_letter() {
        let file = ParsedFile {
            path: "C:\\Project\\Config\\DefaultEngine.ini".into(),
            category: Category::Project,
            sections: vec![ddc_section(&[("Path", "Z:\\DDC")])],
        };
        let findings = run_rules(&file, &EnvVarState::default());
        assert!(findings.iter().any(|f| f.rule_id == "R004" && f.severity == Severity::Warning));
    }

    #[test]
    fn r005_warning_when_deprecated_cvar_present() {
        let file = ParsedFile {
            path: "C:\\Project\\Config\\ConsoleVariables.ini".into(),
            category: Category::Project,
            sections: vec![ParsedSection {
                name: "Startup".into(),
                keys: vec![ParsedKey {
                    name: "r.SShaderCache".into(),
                    value: "1".into(),
                    line_number: 12,
                }],
                backend_nodes: vec![],
            }],
        };
        let findings = run_rules(&file, &EnvVarState::default());
        assert!(findings.iter().any(|f| f.rule_id == "R005" && f.severity == Severity::Warning));
    }

    #[test]
    fn r006_warning_when_envoverride_set_but_envvar_empty() {
        let file = ParsedFile {
            path: "C:\\Project\\Config\\DefaultEngine.ini".into(),
            category: Category::Project,
            sections: vec![ddc_section(&[("EnvPathOverride", "UE-SharedDataCachePath")])],
        };
        let env_state = EnvVarState::default();
        let findings = run_rules(&file, &env_state);
        assert!(findings.iter().any(|f| f.rule_id == "R006" && f.severity == Severity::Warning));
    }

    fn console_variables(keys: &[(&str, &str)]) -> ParsedFile {
        ParsedFile {
            path: "C:\\Project\\Config\\ConsoleVariables.ini".into(),
            category: Category::Project,
            sections: vec![ParsedSection {
                name: "ConsoleVariables".into(),
                keys: keys
                    .iter()
                    .enumerate()
                    .map(|(idx, (k, v))| ParsedKey {
                        name: k.to_string(),
                        value: v.to_string(),
                        line_number: idx + 1,
                    })
                    .collect(),
                backend_nodes: vec![],
            }],
        }
    }

    #[test]
    fn r008_reports_critical_when_pso_precaching_is_missing() {
        let file = console_variables(&[
            ("r.PSOPrecache.Compile", "1"),
            ("r.PSOPrecache.GlobalShaders", "1"),
        ]);
        let findings = run_rules(&file, &EnvVarState::default());
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "R008" && f.severity == Severity::Critical));
    }

    #[test]
    fn r009_reports_warning_when_pso_compile_is_off() {
        let file = console_variables(&[
            ("r.PSOPrecaching", "1"),
            ("r.PSOPrecache.Compile", "0"),
            ("r.PSOPrecache.GlobalShaders", "1"),
        ]);
        let findings = run_rules(&file, &EnvVarState::default());
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "R009" && f.recommended_action == RecommendedAction::Set));
    }

    #[test]
    fn r010_reports_warning_when_global_shader_pso_is_missing() {
        let file = console_variables(&[
            ("r.PSOPrecaching", "1"),
            ("r.PSOPrecache.Compile", "1"),
        ]);
        let findings = run_rules(&file, &EnvVarState::default());
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "R010" && f.recommended_value.as_deref() == Some("1")));
    }

    #[test]
    fn pso_rules_are_clean_when_all_required_cvars_are_enabled() {
        let file = console_variables(&[
            ("r.PSOPrecaching", "1"),
            ("r.PSOPrecache.Compile", "1"),
            ("r.PSOPrecache.GlobalShaders", "1"),
        ]);
        let findings = run_rules(&file, &EnvVarState::default());
        assert!(!findings
            .iter()
            .any(|f| matches!(f.rule_id.as_str(), "R008" | "R009" | "R010")));
    }
}
