//! Pure INI diagnostic rules for DDC and runtime-cache related settings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DDC_SECTION: &str = "/Script/UnrealEd.DerivedDataCacheSettings";
pub const SHARED_DDC_ENV: &str = "UE-SharedDataCachePath";
pub const DEPRECATED_CVARS: &[&str] = &[
    "r.SShaderCache",
    "s.SkipFinalizeCommandList",
    "r.UseShaderCaching",
    "r.UseShaderPredraw",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Healthy,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::Warning => "warning",
            Severity::Healthy => "healthy",
            Severity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IniEntry {
    pub section: String,
    pub key: String,
    pub value: String,
    pub line_number: i64,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IniDocument {
    pub file_path: String,
    pub category: String,
    pub entries: Vec<IniEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    pub rule_id: String,
    pub severity: Severity,
    pub category: String,
    pub file_path: String,
    pub section: Option<String>,
    pub key_name: Option<String>,
    pub line_number: Option<i64>,
    pub snippet_before: String,
    pub snippet_after: Option<String>,
    pub recommended_action: String,
    pub recommended_value: Option<String>,
    pub symptom: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticContext {
    pub env_vars: HashMap<String, Option<String>>,
    pub path_reachability: HashMap<String, bool>,
}

pub fn parse_ini(file_path: &str, category: &str, text: &str) -> IniDocument {
    let mut section = String::new();
    let mut entries = Vec::new();
    for (idx, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed.trim_start_matches('[').trim_end_matches(']').to_string();
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            entries.push(IniEntry {
                section: section.clone(),
                key: trimmed[..eq].trim().to_string(),
                value: strip_inline_comment(trimmed[eq + 1..].trim()).to_string(),
                line_number: idx as i64 + 1,
                raw: raw.to_string(),
            });
        }
    }
    IniDocument {
        file_path: file_path.to_string(),
        category: category.to_string(),
        entries,
    }
}

pub fn diagnose(doc: &IniDocument, ctx: &DiagnosticContext) -> Vec<Finding> {
    let mut findings = Vec::new();
    let ddc_entries: Vec<_> = doc.entries.iter().filter(|e| same_section(&e.section, DDC_SECTION)).collect();
    let path_entry = ddc_entries.iter().find(|e| normalized_key(&e.key) == "Path");
    let env_entry = ddc_entries.iter().find(|e| normalized_key(&e.key) == "EnvPathOverride");

    if let Some(path) = path_entry {
        if env_entry.map(|e| e.value.trim().is_empty()).unwrap_or(true) {
            findings.push(Finding {
                rule_id: "R001".into(),
                severity: Severity::Critical,
                category: doc.category.clone(),
                file_path: doc.file_path.clone(),
                section: Some(path.section.clone()),
                key_name: Some("Path".into()),
                line_number: Some(path.line_number),
                snippet_before: path.raw.clone(),
                snippet_after: Some(format!("EnvPathOverride={}", SHARED_DDC_ENV)),
                recommended_action: "set_env_override_remove_path".into(),
                recommended_value: Some(SHARED_DDC_ENV.into()),
                symptom: "Shared DDC can be bypassed by a hardcoded Path entry.".into(),
                rationale: "EnvPathOverride keeps cache routing controlled by the machine environment instead of stale INI paths.".into(),
            });
        }
        if is_mapped_drive(&path.value) {
            findings.push(mapped_drive_finding(doc, path));
        }
        if ctx.path_reachability.get(&path.value).copied() == Some(false) {
            findings.push(Finding {
                rule_id: "R003".into(),
                severity: Severity::Critical,
                category: doc.category.clone(),
                file_path: doc.file_path.clone(),
                section: Some(path.section.clone()),
                key_name: Some(path.key.clone()),
                line_number: Some(path.line_number),
                snippet_before: path.raw.clone(),
                snippet_after: None,
                recommended_action: "manual".into(),
                recommended_value: None,
                symptom: "Configured DDC path is unreachable from this machine.".into(),
                rationale: "Unreachable cache roots force local fallback and inconsistent derived data.".into(),
            });
        }
    }

    if doc.file_path.ends_with("EditorPerProjectUserSettings.ini") && !ddc_entries.is_empty() {
        let first = ddc_entries[0];
        findings.push(Finding {
            rule_id: "R002".into(),
            severity: Severity::Critical,
            category: doc.category.clone(),
            file_path: doc.file_path.clone(),
            section: Some(first.section.clone()),
            key_name: None,
            line_number: Some(first.line_number),
            snippet_before: ddc_entries.iter().map(|e| e.raw.as_str()).collect::<Vec<_>>().join("\n"),
            snippet_after: Some("; remove user-level DerivedDataCacheSettings override".into()),
            recommended_action: "manual".into(),
            recommended_value: None,
            symptom: "User-level DDC override can shadow project and engine settings.".into(),
            rationale: "Per-project user settings should not define shared DDC policy.".into(),
        });
    }

    for entry in &doc.entries {
        let key = normalized_key(&entry.key);
        if DEPRECATED_CVARS.iter().any(|c| *c == key) {
            findings.push(Finding {
                rule_id: "R005".into(),
                severity: Severity::Warning,
                category: doc.category.clone(),
                file_path: doc.file_path.clone(),
                section: Some(entry.section.clone()),
                key_name: Some(entry.key.clone()),
                line_number: Some(entry.line_number),
                snippet_before: entry.raw.clone(),
                snippet_after: None,
                recommended_action: "remove".into(),
                recommended_value: None,
                symptom: "Deprecated shader cache CVar may conflict with modern UE cache behavior.".into(),
                rationale: "These CVars are legacy-era toggles and should not drive current DDC/PSO workflows.".into(),
            });
        }
    }

    diagnose_pso_precaching(doc, &mut findings);

    if let Some(env) = env_entry {
        let env_name = env.value.trim();
        let env_value = ctx.env_vars.get(env_name).and_then(|v| v.as_deref()).unwrap_or("");
        if env_value.is_empty() {
            findings.push(Finding {
                rule_id: "R006".into(),
                severity: Severity::Warning,
                category: doc.category.clone(),
                file_path: doc.file_path.clone(),
                section: Some(env.section.clone()),
                key_name: Some(env.key.clone()),
                line_number: Some(env.line_number),
                snippet_before: env.raw.clone(),
                snippet_after: None,
                recommended_action: "manual".into(),
                recommended_value: Some(env_name.into()),
                symptom: "INI references an environment variable that is not set on this machine.".into(),
                rationale: "EnvPathOverride only works when the named machine-level variable resolves to a cache path.".into(),
            });
        } else if env_name == SHARED_DDC_ENV && ctx.path_reachability.get(env_value).copied().unwrap_or(true) {
            findings.push(Finding {
                rule_id: "R007".into(),
                severity: Severity::Healthy,
                category: doc.category.clone(),
                file_path: doc.file_path.clone(),
                section: Some(env.section.clone()),
                key_name: Some(env.key.clone()),
                line_number: Some(env.line_number),
                snippet_before: env.raw.clone(),
                snippet_after: None,
                recommended_action: "none".into(),
                recommended_value: Some(env_value.into()),
                symptom: "Shared DDC is routed through the expected environment variable.".into(),
                rationale: "The machine has a configured shared cache path and the INI uses EnvPathOverride.".into(),
            });
        }
    }

    findings
}

fn diagnose_pso_precaching(doc: &IniDocument, findings: &mut Vec<Finding>) {
    if !doc.file_path.to_lowercase().ends_with("consolevariables.ini") {
        return;
    }
    let entries: Vec<_> = doc
        .entries
        .iter()
        .filter(|entry| same_section(&entry.section, "ConsoleVariables"))
        .collect();
    findings.extend(pso_cvar_rule(
        doc,
        &entries,
        "R008",
        "r.PSOPrecaching",
        Severity::Critical,
        "PSO precaching is disabled or not configured.",
        "Runtime PSO precaching must be enabled before collecting and distributing useful PSO cache files.",
    ));
    findings.extend(pso_cvar_rule(
        doc,
        &entries,
        "R009",
        "r.PSOPrecache.Compile",
        Severity::Warning,
        "PSO precache compilation is disabled or not configured.",
        "UE versions and project configs can leave compile behavior disabled unless explicitly set.",
    ));
    findings.extend(pso_cvar_rule(
        doc,
        &entries,
        "R010",
        "r.PSOPrecache.GlobalShaders",
        Severity::Warning,
        "Global shader PSO precaching is disabled or not configured.",
        "Global shader PSOs should be covered so first scene switches do not create avoidable hitches.",
    ));
}

fn pso_cvar_rule(
    doc: &IniDocument,
    entries: &[&IniEntry],
    rule_id: &str,
    key_name: &str,
    severity: Severity,
    symptom: &str,
    rationale: &str,
) -> Option<Finding> {
    let found = entries
        .iter()
        .find(|entry| normalized_key(&entry.key).eq_ignore_ascii_case(key_name));
    if found.map(|entry| truthy(&entry.value)).unwrap_or(false) {
        return None;
    }
    Some(Finding {
        rule_id: rule_id.into(),
        severity,
        category: doc.category.clone(),
        file_path: doc.file_path.clone(),
        section: Some("ConsoleVariables".into()),
        key_name: Some(key_name.into()),
        line_number: found.map(|entry| entry.line_number),
        snippet_before: found
            .map(|entry| entry.raw.clone())
            .unwrap_or_else(|| "[ConsoleVariables]".into()),
        snippet_after: Some(format!("{}=1", key_name)),
        recommended_action: "set".into(),
        recommended_value: Some("1".into()),
        symptom: symptom.into(),
        rationale: rationale.into(),
    })
}

fn mapped_drive_finding(doc: &IniDocument, entry: &IniEntry) -> Finding {
    Finding {
        rule_id: "R004".into(),
        severity: Severity::Warning,
        category: doc.category.clone(),
        file_path: doc.file_path.clone(),
        section: Some(entry.section.clone()),
        key_name: Some(entry.key.clone()),
        line_number: Some(entry.line_number),
        snippet_before: entry.raw.clone(),
        snippet_after: None,
        recommended_action: "manual".into(),
        recommended_value: None,
        symptom: "Mapped drive paths are user-session specific and may not exist under SYSTEM or services.".into(),
        rationale: "Use UNC paths for cross-machine cache paths.".into(),
    }
}

fn same_section(left: &str, right: &str) -> bool {
    left.trim_matches(&['[', ']'][..]).eq_ignore_ascii_case(right)
}

pub fn normalized_key(key: &str) -> &str {
    key.trim_start_matches(['+', '-', '!'])
}

fn is_mapped_drive(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') && bytes[0].is_ascii_alphabetic()
}

fn strip_inline_comment(value: &str) -> &str {
    for token in [" ;", " //"] {
        if let Some(pos) = value.find(token) {
            return value[..pos].trim();
        }
    }
    value
}

fn truthy(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with_env(value: Option<&str>) -> DiagnosticContext {
        let mut ctx = DiagnosticContext::default();
        ctx.env_vars.insert(SHARED_DDC_ENV.into(), value.map(str::to_string));
        ctx
    }

    #[test]
    fn r001_detects_hardcoded_path_without_env_override() {
        let doc = parse_ini("DefaultEngine.ini", "project", "[/Script/UnrealEd.DerivedDataCacheSettings]\nPath=D:\\Old");
        let findings = diagnose(&doc, &DiagnosticContext::default());
        assert!(findings.iter().any(|f| f.rule_id == "R001" && f.severity == Severity::Critical));
    }

    #[test]
    fn r002_detects_user_level_ddc_override() {
        let doc = parse_ini("EditorPerProjectUserSettings.ini", "user", "[/Script/UnrealEd.DerivedDataCacheSettings]\nPath=C:\\local");
        assert!(diagnose(&doc, &DiagnosticContext::default()).iter().any(|f| f.rule_id == "R002"));
    }

    #[test]
    fn r004_detects_mapped_drive_path() {
        let doc = parse_ini("DefaultEngine.ini", "project", "[/Script/UnrealEd.DerivedDataCacheSettings]\nPath=Z:\\DDC");
        assert!(diagnose(&doc, &DiagnosticContext::default()).iter().any(|f| f.rule_id == "R004"));
    }

    #[test]
    fn r005_detects_deprecated_cvar() {
        let doc = parse_ini("ConsoleVariables.ini", "project", "[Startup]\nr.SShaderCache=1");
        assert!(diagnose(&doc, &DiagnosticContext::default()).iter().any(|f| f.rule_id == "R005"));
    }

    #[test]
    fn r006_detects_missing_env_var() {
        let doc = parse_ini("DefaultEngine.ini", "project", "[/Script/UnrealEd.DerivedDataCacheSettings]\nEnvPathOverride=UE-SharedDataCachePath");
        assert!(diagnose(&doc, &DiagnosticContext::default()).iter().any(|f| f.rule_id == "R006"));
    }

    #[test]
    fn r007_reports_healthy_env_override() {
        let doc = parse_ini("DefaultEngine.ini", "project", "[/Script/UnrealEd.DerivedDataCacheSettings]\nEnvPathOverride=UE-SharedDataCachePath");
        assert!(diagnose(&doc, &ctx_with_env(Some("\\\\HOST\\DDC"))).iter().any(|f| f.rule_id == "R007"));
    }

    #[test]
    fn r008_reports_critical_when_pso_precaching_is_missing() {
        let doc = parse_ini(
            "C:\\Project\\Config\\ConsoleVariables.ini",
            "project",
            "[ConsoleVariables]\nr.PSOPrecache.Compile=1\nr.PSOPrecache.GlobalShaders=1",
        );
        let findings = diagnose(&doc, &DiagnosticContext::default());
        assert!(findings.iter().any(|finding| finding.rule_id == "R008" && finding.severity == Severity::Critical));
    }

    #[test]
    fn r009_reports_warning_when_pso_compile_is_off() {
        let doc = parse_ini(
            "C:\\Project\\Config\\ConsoleVariables.ini",
            "project",
            "[ConsoleVariables]\nr.PSOPrecaching=1\nr.PSOPrecache.Compile=0\nr.PSOPrecache.GlobalShaders=1",
        );
        let findings = diagnose(&doc, &DiagnosticContext::default());
        assert!(findings.iter().any(|finding| finding.rule_id == "R009" && finding.recommended_action == "set"));
    }

    #[test]
    fn r010_reports_warning_when_global_shader_precache_is_missing() {
        let doc = parse_ini(
            "C:\\Project\\Config\\ConsoleVariables.ini",
            "project",
            "[ConsoleVariables]\nr.PSOPrecaching=1\nr.PSOPrecache.Compile=1",
        );
        let findings = diagnose(&doc, &DiagnosticContext::default());
        assert!(findings.iter().any(|finding| finding.rule_id == "R010" && finding.recommended_value.as_deref() == Some("1")));
    }

    #[test]
    fn pso_rules_are_clean_when_all_required_cvars_are_enabled() {
        let doc = parse_ini(
            "C:\\Project\\Config\\ConsoleVariables.ini",
            "project",
            "[ConsoleVariables]\nr.PSOPrecaching=1\nr.PSOPrecache.Compile=1\nr.PSOPrecache.GlobalShaders=1",
        );
        let findings = diagnose(&doc, &DiagnosticContext::default());
        assert!(!findings.iter().any(|finding| matches!(finding.rule_id.as_str(), "R008" | "R009" | "R010")));
    }
}
