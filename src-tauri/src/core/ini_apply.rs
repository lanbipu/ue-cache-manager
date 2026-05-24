//! Translate a single ini_findings row into a concrete ini_editor call.

use crate::core::ini_editor;
use crate::data::ini_findings::IniFinding;
use crate::error::{UecmError, UecmResult};

pub struct ApplyContext<'a> {
    pub host: &'a str,
}

pub fn apply(ctx: &ApplyContext, finding: &IniFinding) -> UecmResult<String> {
    let section = finding.section.as_deref()
        .ok_or_else(|| UecmError::InvalidInput("finding has no section".into()))?;
    match finding.recommended_action.as_str() {
        "set" => {
            let key = finding.key_name.as_deref()
                .ok_or_else(|| UecmError::InvalidInput("finding has no key_name".into()))?;
            let value = finding.recommended_value.as_deref()
                .ok_or_else(|| UecmError::InvalidInput("finding has no recommended_value".into()))?;
            if finding.rule_id == "R001" {
                ini_editor::remove_key(ctx.host, &finding.file_path, section, key)?;
                return ini_editor::set_key(
                    ctx.host, &finding.file_path, section, "EnvPathOverride", value,
                );
            }
            ini_editor::set_key(ctx.host, &finding.file_path, section, key, value)
        }
        "remove" => {
            if finding.rule_id == "R002" {
                for line in finding.snippet_before.lines() {
                    if let Some(eq) = line.find('=') {
                        let key = line[..eq].trim();
                        if !key.is_empty() {
                            ini_editor::remove_key(ctx.host, &finding.file_path, section, key)?;
                        }
                    }
                }
                return Ok("multiple-key removal".into());
            }
            let key = finding.key_name.as_deref()
                .ok_or_else(|| UecmError::InvalidInput("remove needs key_name".into()))?;
            ini_editor::remove_key(ctx.host, &finding.file_path, section, key)
        }
        "manual" => Err(UecmError::InvalidInput(
            "manual findings cannot be auto-applied; open the file directly".into(),
        )),
        other => Err(UecmError::InvalidInput(format!("unknown action: {}", other))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::ini_findings::IniFinding;

    fn finding(action: &str, rule: &str) -> IniFinding {
        IniFinding {
            id: Some(1), scan_run_id: 1, machine_id: 1,
            rule_id: rule.into(), severity: "critical".into(),
            category: "project".into(),
            file_path: "C:\\f.ini".into(),
            section: Some("DDC".into()), key_name: Some("Path".into()),
            line_number: Some(1),
            snippet_before: "Path=X".into(), snippet_after: None,
            recommended_action: action.into(), recommended_value: Some("V".into()),
            symptom: "".into(), rationale: "".into(),
            fixed_at: None, skipped_at: None,
        }
    }

    #[test]
    fn manual_action_returns_invalid_input() {
        let ctx = ApplyContext { host: "X" };
        let result = apply(&ctx, &finding("manual", "R004"));
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn unknown_action_returns_invalid_input() {
        let ctx = ApplyContext { host: "X" };
        let result = apply(&ctx, &finding("zzz", "R999"));
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }
}
