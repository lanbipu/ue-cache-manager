//! Applies selected INI findings through the existing atomic INI write path.

use crate::core::ini_diagnostics;
use crate::core::ini_editor;
use crate::data::{self, Db};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplyFindingResult {
    pub backup_path: Option<String>,
    pub message: String,
}

pub fn apply(db: &Db, finding_id: i64, credential_alias: &str) -> UecmResult<ApplyFindingResult> {
    let finding = data::ini_findings::find_by_id(db, finding_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("finding {} not found", finding_id)))?;
    let machine = data::machines::find_by_id(db, finding.machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", finding.machine_id)))?;
    let credential = data::credentials::find_by_alias(db, credential_alias)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("credential alias '{}' not found", credential_alias))
    })?;
    let password = crate::core::credentials::resolve_password(credential_alias)?;
    let section = finding.section.clone().ok_or_else(|| {
        UecmError::InvalidInput("finding has no section and cannot be auto-applied".into())
    })?;

    let backup_path = match finding.recommended_action.as_str() {
        "set" => {
            let key = finding.key_name.as_deref().unwrap_or("EnvPathOverride");
            let value = finding.recommended_value.as_deref().ok_or_else(|| {
                UecmError::InvalidInput("set finding has no recommended value".into())
            })?;
            let backup = ini_editor::set_key_with_credential(
                &machine.ip,
                &finding.file_path,
                &section,
                key,
                value,
                &credential.username,
                &password,
            )?;
            let keys = ini_editor::read_section_with_credential(
                &machine.ip,
                &finding.file_path,
                &section,
                &credential.username,
                &password,
            )?;
            verify_key_value(&keys, key, value)?;
            Some(backup)
        }
        "remove" => {
            let key = finding.key_name.as_deref().ok_or_else(|| {
                UecmError::InvalidInput("remove finding has no key name".into())
            })?;
            let normalized = ini_diagnostics::normalized_key(key);
            let backup = ini_editor::remove_key_with_credential(
                &machine.ip,
                &finding.file_path,
                &section,
                normalized,
                &credential.username,
                &password,
            )?;
            let keys = ini_editor::read_section_with_credential(
                &machine.ip,
                &finding.file_path,
                &section,
                &credential.username,
                &password,
            )?;
            verify_key_absent(&keys, normalized)?;
            Some(backup)
        }
        "set_env_override_remove_path" => {
            let set_backup = ini_editor::set_key_with_credential(
                &machine.ip,
                &finding.file_path,
                &section,
                "EnvPathOverride",
                finding.recommended_value.as_deref().unwrap_or(ini_diagnostics::SHARED_DDC_ENV),
                &credential.username,
                &password,
            )?;
            let _ = ini_editor::remove_key_with_credential(
                &machine.ip,
                &finding.file_path,
                &section,
                "Path",
                &credential.username,
                &password,
            )?;
            let keys = ini_editor::read_section_with_credential(
                &machine.ip,
                &finding.file_path,
                &section,
                &credential.username,
                &password,
            )?;
            verify_key_value(
                &keys,
                "EnvPathOverride",
                finding.recommended_value.as_deref().unwrap_or(ini_diagnostics::SHARED_DDC_ENV),
            )?;
            verify_key_absent(&keys, "Path")?;
            Some(set_backup)
        }
        other => {
            return Err(UecmError::InvalidInput(format!(
                "finding action '{}' is not auto-applicable",
                other
            )))
        }
    };

    data::ini_findings::mark_fixed(db, finding_id)?;
    Ok(ApplyFindingResult {
        backup_path,
        message: "applied".into(),
    })
}

fn verify_key_value(keys: &[ini_editor::IniKey], name: &str, expected: &str) -> UecmResult<()> {
    let Some(found) = keys
        .iter()
        .find(|key| ini_diagnostics::normalized_key(&key.name).eq_ignore_ascii_case(name))
    else {
        return Err(UecmError::OperationFailed(format!(
            "verification failed: key '{}' was not found after apply",
            name
        )));
    };
    if found.value.trim() != expected {
        return Err(UecmError::OperationFailed(format!(
            "verification failed: key '{}' value is '{}', expected '{}'",
            name, found.value, expected
        )));
    }
    Ok(())
}

fn verify_key_absent(keys: &[ini_editor::IniKey], name: &str) -> UecmResult<()> {
    if keys
        .iter()
        .any(|key| ini_diagnostics::normalized_key(&key.name).eq_ignore_ascii_case(name))
    {
        return Err(UecmError::OperationFailed(format!(
            "verification failed: key '{}' still exists after apply",
            name
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_key_value_accepts_matching_key() {
        let keys = vec![ini_editor::IniKey {
            name: "EnvPathOverride".into(),
            value: ini_diagnostics::SHARED_DDC_ENV.into(),
        }];
        verify_key_value(&keys, "EnvPathOverride", ini_diagnostics::SHARED_DDC_ENV).unwrap();
    }

    #[test]
    fn verify_key_absent_rejects_prefixed_key() {
        let keys = vec![ini_editor::IniKey {
            name: "+Path".into(),
            value: "D:\\Old".into(),
        }];
        assert!(verify_key_absent(&keys, "Path").is_err());
    }
}
