//! Single-machine INI section read + key write via PowerShell sidecar.

use crate::core::{loopback, powershell};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IniKey {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadResult {
    pub ok: bool,
    pub keys: Vec<IniKey>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct WriteResult {
    pub ok: bool,
    pub backup_path: String,
    pub message: String,
}

pub fn read_section(host: &str, file_path: &str, section: &str) -> UecmResult<Vec<IniKey>> {
    if loopback::is_loopback_target(host) {
        return read_section_local(file_path, section);
    }

    let result: ReadResult = powershell::run_json(
        &powershell::script_path("read-ini-section.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Section", section,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "read INI failed: {}",
            result.message
        )));
    }
    Ok(result.keys)
}

pub fn set_key(
    host: &str,
    file_path: &str,
    section: &str,
    name: &str,
    value: &str,
) -> UecmResult<String> {
    if loopback::is_loopback_target(host) {
        return write_key_local(file_path, section, name, Some(value));
    }

    let result: WriteResult = powershell::run_json(
        &powershell::script_path("write-ini-key.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Section", section,
            "-Name", name,
            "-Value", value,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "write INI failed: {}",
            result.message
        )));
    }
    Ok(result.backup_path)
}

/// Same as `read_section`, but authenticates the WinRM session with explicit
/// `username` + `password` instead of inheriting the caller's identity.
pub fn read_section_with_credential(
    host: &str,
    file_path: &str,
    section: &str,
    username: &str,
    password: &str,
) -> UecmResult<Vec<IniKey>> {
    if loopback::is_loopback_target(host) {
        let _ = (username, password);
        return read_section_local(file_path, section);
    }

    let result: ReadResult = powershell::run_json(
        &powershell::script_path("read-ini-section.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Section", section,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "read INI failed: {}",
            result.message
        )));
    }
    Ok(result.keys)
}

/// Same as `set_key`, but authenticates the WinRM session with explicit
/// `username` + `password` instead of inheriting the caller's identity.
pub fn set_key_with_credential(
    host: &str,
    file_path: &str,
    section: &str,
    name: &str,
    value: &str,
    username: &str,
    password: &str,
) -> UecmResult<String> {
    if loopback::is_loopback_target(host) {
        let _ = (username, password);
        return write_key_local(file_path, section, name, Some(value));
    }

    let result: WriteResult = powershell::run_json(
        &powershell::script_path("write-ini-key.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Section", section,
            "-Name", name,
            "-Value", value,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "write INI failed: {}",
            result.message
        )));
    }
    Ok(result.backup_path)
}

/// Removes a key from an INI section on a remote host. Authenticates with
/// explicit `username` + `password`. Returns the backup path created by the
/// PS sidecar.
pub fn remove_key_with_credential(
    host: &str,
    file_path: &str,
    section: &str,
    name: &str,
    username: &str,
    password: &str,
) -> UecmResult<String> {
    if loopback::is_loopback_target(host) {
        let _ = (username, password);
        return write_key_local(file_path, section, name, None);
    }

    let result: WriteResult = powershell::run_json(
        &powershell::script_path("write-ini-key.ps1"),
        &[
            "-HostName", host, "-FilePath", file_path, "-Section", section,
            "-Name", name, "-RemoveKey",
            "-Username", username, "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "remove key failed: {}",
            result.message
        )));
    }
    Ok(result.backup_path)
}

fn read_section_local(file_path: &str, section: &str) -> UecmResult<Vec<IniKey>> {
    let contents = std::fs::read_to_string(file_path)?;
    let mut keys = Vec::new();
    let mut in_section = false;
    let section_marker = format!("[{}]", section);

    for line in contents.lines() {
        let trim = line.trim();
        if trim.eq_ignore_ascii_case(&section_marker) {
            in_section = true;
            continue;
        }
        if in_section && trim.starts_with('[') && trim.ends_with(']') {
            break;
        }
        if in_section
            && !trim.is_empty()
            && !trim.starts_with(';')
            && !trim.starts_with('#')
        {
            if let Some(eq) = trim.find('=') {
                if eq > 0 {
                    keys.push(IniKey {
                        name: trim[..eq].trim().to_string(),
                        value: trim[eq + 1..].trim().to_string(),
                    });
                }
            }
        }
    }

    Ok(keys)
}

fn write_key_local(
    file_path: &str,
    section: &str,
    name: &str,
    value: Option<&str>,
) -> UecmResult<String> {
    let contents = std::fs::read_to_string(file_path)?;
    let backup = local_backup_path(file_path);
    std::fs::copy(file_path, &backup)?;

    let remove = value.is_none();
    let mut out = Vec::new();
    let mut in_section = false;
    let mut section_seen = false;
    let mut written = false;
    let section_marker = format!("[{}]", section);

    for line in contents.lines() {
        let trim = line.trim();
        if trim.eq_ignore_ascii_case(&section_marker) {
            in_section = true;
            section_seen = true;
            out.push(line.to_string());
            continue;
        }
        if in_section && trim.starts_with('[') && trim.ends_with(']') {
            if !remove && !written {
                out.push(format!("{}={}", name, value.unwrap_or_default()));
                written = true;
            }
            in_section = false;
            out.push(line.to_string());
            continue;
        }
        if in_section && key_matches(trim, name) {
            if remove {
                continue;
            }
            out.push(format!("{}={}", name, value.unwrap_or_default()));
            written = true;
            continue;
        }
        out.push(line.to_string());
    }

    if !remove && !written && in_section {
        out.push(format!("{}={}", name, value.unwrap_or_default()));
    }

    if !remove && !section_seen {
        if out.last().is_some_and(|line| !line.trim().is_empty()) {
            out.push(String::new());
        }
        out.push(section_marker);
        out.push(format!("{}={}", name, value.unwrap_or_default()));
    }

    let mut updated = out.join("\n");
    if contents.ends_with('\n') || !updated.is_empty() {
        updated.push('\n');
    }
    std::fs::write(file_path, updated)?;

    Ok(backup)
}

fn key_matches(trimmed_line: &str, name: &str) -> bool {
    trimmed_line
        .find('=')
        .map(|eq| trimmed_line[..eq].trim() == name)
        .unwrap_or(false)
}

fn local_backup_path(file_path: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{}.bak.{}", file_path, millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn read_section_returns_powershell_error_on_non_windows() {
        let result = read_section("RENDER-01", "C:\\proj\\Config\\DefaultEngine.ini", "Core.System");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn set_key_returns_powershell_error_on_non_windows() {
        let result = set_key(
            "RENDER-01",
            "C:\\proj\\Config\\DefaultEngine.ini",
            "Core.System",
            "Paths",
            "../Content",
        );
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn read_section_with_credential_returns_powershell_error_on_non_windows() {
        let result = read_section_with_credential(
            "RENDER-01",
            "C:\\proj\\Config\\DefaultEngine.ini",
            "Core.System",
            "admin",
            "p@ss",
        );
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn set_key_with_credential_returns_powershell_error_on_non_windows() {
        let result = set_key_with_credential(
            "RENDER-01",
            "C:\\proj\\Config\\DefaultEngine.ini",
            "Core.System",
            "Paths",
            "../Content",
            "admin",
            "p@ss",
        );
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn remove_key_with_credential_returns_powershell_error_on_non_windows() {
        let result = remove_key_with_credential(
            "h",
            "f",
            "s",
            "k",
            "u",
            "p",
        );
        assert!(result.is_err());
    }

    #[test]
    fn set_key_with_credential_writes_directly_for_loopback_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DefaultEngine.ini");
        std::fs::write(&path, "[DDC]\nPath=Old\n").unwrap();

        let backup = set_key_with_credential(
            "localhost",
            &path.to_string_lossy(),
            "DDC",
            "Path",
            "New",
            "ignored",
            "ignored",
        )
        .unwrap();

        let updated = std::fs::read_to_string(&path).unwrap();
        assert!(updated.contains("Path=New"));
        assert!(std::path::Path::new(&backup).exists());
    }

    #[test]
    fn read_section_local_is_case_insensitive() {
        // The file uses [DDC] but caller asks for "ddc" — should still match,
        // matching the remote PowerShell path's case-insensitive comparison.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DefaultEngine.ini");
        std::fs::write(&path, "[DDC]\nPath=Old\nSize=1024\n").unwrap();

        let keys = read_section_local(path.to_str().unwrap(), "ddc").unwrap();
        assert!(keys.iter().any(|k| k.name == "Path" && k.value == "Old"));
        assert!(keys.iter().any(|k| k.name == "Size" && k.value == "1024"));
    }

    #[test]
    fn set_key_with_credential_section_match_is_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DefaultEngine.ini");
        std::fs::write(&path, "[DDC]\nPath=Old\n").unwrap();

        set_key_with_credential(
            "localhost", &path.to_string_lossy(),
            "ddc", "Path", "New",
            "ignored", "ignored",
        ).unwrap();

        let updated = std::fs::read_to_string(&path).unwrap();
        assert!(updated.contains("[DDC]"), "original section header preserved");
        assert!(updated.contains("Path=New"));
        assert!(!updated.contains("Path=Old"));
    }

    #[test]
    fn remove_key_with_credential_writes_directly_for_loopback_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DefaultEngine.ini");
        std::fs::write(&path, "[DDC]\nPath=Old\nKeep=1\n").unwrap();

        remove_key_with_credential(
            "localhost",
            &path.to_string_lossy(),
            "DDC",
            "Path",
            "ignored",
            "ignored",
        )
        .unwrap();

        let updated = std::fs::read_to_string(&path).unwrap();
        assert!(!updated.contains("Path=Old"));
        assert!(updated.contains("Keep=1"));
    }
}
