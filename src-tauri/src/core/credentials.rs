//! Wraps the cred-{set,delete,list}.ps1 sidecar scripts. Stores the alias +
//! display username in SQLite (via `data::credentials`); the password lives
//! in Windows Credential Manager (cmdkey, used for transparent SMB auth) and
//! ALSO in a DPAPI-encrypted form on disk so the Rust side can read it back
//! when an explicit credential needs to be passed into Invoke-Command.
//!
//! Storage layout for the DPAPI-encrypted half:
//!   - File: `%LOCALAPPDATA%\UECM\creds.bin` (Windows) — JSON object
//!     `{ alias: ciphertext_base64 }`. Whole file rewritten on each store/delete.
//!   - Each entry is encrypted via `ps-scripts/dpapi.ps1` which delegates to
//!     .NET System.Security.Cryptography.ProtectedData (CurrentUser scope).
//!     Going through PS instead of a windows-rs FFI binding keeps the Rust
//!     side dep-free and survives windows-rs API drift.
//!
//! Non-Windows: `store_password` and `delete_password` are no-ops returning
//! Ok(()); `resolve_password` is SecretStore-first (cross-platform) and only
//! falls back to the Windows DPAPI store, so off Windows it reads the SecretStore.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CmdKeyResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CmdKeyAlias {
    pub alias: String,
}

pub fn normalize_username_for_storage(username: &str) -> String {
    let trimmed = username.trim();
    trimmed
        .strip_prefix(".\\")
        .or_else(|| trimmed.strip_prefix("./"))
        .unwrap_or(trimmed)
        .to_string()
}

pub fn store(alias: &str, username: &str, password: &str) -> UecmResult<()> {
    let result: CmdKeyResult = powershell::run_json(
        &powershell::script_path("cred-set.ps1"),
        &[
            "-Alias", alias,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "cred-set failed: {}",
            result.message
        )));
    }
    Ok(())
}

pub fn delete(alias: &str) -> UecmResult<()> {
    let result: CmdKeyResult = powershell::run_json(
        &powershell::script_path("cred-delete.ps1"),
        &["-Alias", alias],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "cred-delete failed: {}",
            result.message
        )));
    }
    Ok(())
}

pub fn list_uecm_aliases() -> UecmResult<Vec<String>> {
    let result: Vec<CmdKeyAlias> = powershell::run_json(
        &powershell::script_path("cred-list.ps1"),
        &[],
    )?;
    Ok(result.into_iter().map(|c| c.alias).collect())
}

// ---------------------------------------------------------------------------
// DPAPI-backed password storage
// ---------------------------------------------------------------------------

/// Encrypt + persist `password` for `alias`. Whole file is rewritten on each
/// call. Windows-only at runtime; on other platforms this is a no-op so the
/// dev-box `save_credential` flow doesn't blow up.
#[cfg(windows)]
pub fn store_password(alias: &str, password: &str) -> UecmResult<()> {
    let ciphertext = dpapi::protect(password.as_bytes())?;
    let encoded = base64_encode(&ciphertext);
    let mut store = read_store()?;
    store.insert(alias.to_string(), encoded);
    write_store(&store)
}

#[cfg(not(windows))]
pub fn store_password(alias: &str, password: &str) -> UecmResult<()> {
    let _ = (alias, password);
    Ok(())
}

/// Decrypt + return `password` for `alias`. Returns the **plaintext** so it
/// can be forwarded to a single PowerShell invocation; callers must not log
/// or otherwise persist the result.
#[cfg(windows)]
pub fn resolve_password(alias: &str) -> UecmResult<String> {
    // SecretStore (cross-platform, AES-GCM) is the home for every credential saved
    // since the SSH migration; fall back to the legacy DPAPI store for aliases
    // saved before it. This keeps the still-registered DPAPI consumers
    // (bootstrap_winrm, deploy_ddc_run, machine authorize) working for a
    // freshly-saved alias. The DPAPI store + this fallback retire in P5b.
    if let Some(secret) = crate::core::secrets::SecretStore::from_config()?.get(alias)? {
        return Ok(secret);
    }
    let store = read_store()?;
    let encoded = store.get(alias).ok_or_else(|| {
        UecmError::OperationFailed(format!("no stored secret for alias '{}'", alias))
    })?;
    let ciphertext = base64_decode(encoded)?;
    let plaintext = dpapi::unprotect(&ciphertext)?;
    String::from_utf8(plaintext).map_err(|e| {
        UecmError::OperationFailed(format!("DPAPI plaintext is not valid UTF-8: {}", e))
    })
}

#[cfg(not(windows))]
pub fn resolve_password(alias: &str) -> UecmResult<String> {
    // No DPAPI off Windows — the cross-platform SecretStore is the only home.
    crate::core::secrets::SecretStore::from_config()?
        .get(alias)?
        .ok_or_else(|| UecmError::OperationFailed(format!("no stored secret for alias '{}'", alias)))
}

/// Remove the DPAPI entry for `alias`. Missing entry is not an error.
#[cfg(windows)]
pub fn delete_password(alias: &str) -> UecmResult<()> {
    let mut store = read_store()?;
    if store.remove(alias).is_some() {
        write_store(&store)?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn delete_password(alias: &str) -> UecmResult<()> {
    let _ = alias;
    Ok(())
}

// ---------------------------------------------------------------------------
// JSON store helpers (Windows-only — non-Windows paths return early above)
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn store_path() -> UecmResult<std::path::PathBuf> {
    let base = std::env::var("LOCALAPPDATA").map_err(|_| {
        UecmError::OperationFailed("LOCALAPPDATA env var not set".to_string())
    })?;
    let dir = std::path::PathBuf::from(base).join("UECM");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("creds.bin"))
}

#[cfg(windows)]
fn read_store() -> UecmResult<std::collections::BTreeMap<String, String>> {
    let path = store_path()?;
    if !path.exists() {
        return Ok(std::collections::BTreeMap::new());
    }
    let bytes = std::fs::read(&path)?;
    if bytes.is_empty() {
        return Ok(std::collections::BTreeMap::new());
    }
    serde_json::from_slice(&bytes).map_err(|e| {
        UecmError::OperationFailed(format!("failed to parse DPAPI store: {}", e))
    })
}

#[cfg(windows)]
fn write_store(store: &std::collections::BTreeMap<String, String>) -> UecmResult<()> {
    let path = store_path()?;
    let bytes = serde_json::to_vec(store).map_err(|e| {
        UecmError::OperationFailed(format!("failed to serialize DPAPI store: {}", e))
    })?;
    std::fs::write(&path, bytes)?;
    Ok(())
}

#[cfg(windows)]
fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(windows)]
fn base64_decode(s: &str) -> UecmResult<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| UecmError::OperationFailed(format!("base64 decode failed: {}", e)))
}

// ---------------------------------------------------------------------------
// DPAPI via ps-scripts/dpapi.ps1 (Windows-only at runtime)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod dpapi {
    use crate::core::powershell;
    use crate::error::{UecmError, UecmResult};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct DpapiResult {
        ok: bool,
        data: String,
        message: String,
    }

    fn invoke(mode: &str, data_b64: &str) -> UecmResult<String> {
        let result: DpapiResult = powershell::run_json(
            &powershell::script_path("dpapi.ps1"),
            &["-Mode", mode, "-DataB64", data_b64],
        )?;
        if !result.ok {
            return Err(UecmError::OperationFailed(format!(
                "DPAPI {} failed: {}",
                mode, result.message
            )));
        }
        Ok(result.data)
    }

    pub fn protect(plaintext: &[u8]) -> UecmResult<Vec<u8>> {
        let plaintext_b64 = super::base64_encode(plaintext);
        let out_b64 = invoke("protect", &plaintext_b64)?;
        super::base64_decode(&out_b64)
    }

    pub fn unprotect(ciphertext: &[u8]) -> UecmResult<Vec<u8>> {
        let ciphertext_b64 = super::base64_encode(ciphertext);
        let out_b64 = invoke("unprotect", &ciphertext_b64)?;
        super::base64_decode(&out_b64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn store_returns_powershell_error_on_non_windows() {
        let result = store("UECM:winrm:HOST", "admin", "p@ss");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn delete_returns_powershell_error_on_non_windows() {
        let result = delete("UECM:winrm:HOST");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn list_returns_powershell_error_on_non_windows() {
        let result = list_uecm_aliases();
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    // (Removed `resolve_password_returns_powershell_error_on_non_windows`:
    // resolve_password is now SecretStore-first and cross-platform — off Windows
    // it reads the SecretStore, not DPAPI. The SecretStore read/miss behavior is
    // covered hermetically in core::secrets tests.)

    #[cfg(not(windows))]
    #[test]
    fn store_password_is_noop_on_non_windows() {
        let result = store_password("UECM:winrm:HOST", "p@ss");
        assert!(result.is_ok());
    }

    #[cfg(not(windows))]
    #[test]
    fn delete_password_is_noop_on_non_windows() {
        let result = delete_password("UECM:winrm:HOST");
        assert!(result.is_ok());
    }

    #[test]
    fn normalize_username_for_storage_strips_dot_slash_prefix() {
        assert_eq!(normalize_username_for_storage(".\\uecm-test"), "uecm-test");
        assert_eq!(normalize_username_for_storage("./uecm-test"), "uecm-test");
    }

    #[test]
    fn normalize_username_for_storage_preserves_domain_and_upn() {
        assert_eq!(normalize_username_for_storage("LANPC\\uecm-test"), "LANPC\\uecm-test");
        assert_eq!(normalize_username_for_storage("uecm-test@example.local"), "uecm-test@example.local");
    }
}
