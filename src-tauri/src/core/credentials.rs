//! Wraps the cred-{set,delete,list}.ps1 sidecar scripts. Stores the alias +
//! display username in SQLite (via `data::credentials`); the password lives
//! in Windows Credential Manager (cmdkey, used for transparent SMB auth) and
//! ALSO in DPAPI-encrypted form on disk so the Rust side can read it back
//! when an explicit credential needs to be passed into Invoke-Command.
//!
//! Storage layout for the DPAPI-encrypted half:
//!   - File: `%LOCALAPPDATA%\UECM\creds.bin` (Windows) — JSON object
//!     `{ alias: ciphertext_base64 }`. Whole file rewritten on each store/delete.
//!   - Each entry is `CryptProtectData(password_bytes)` + base64.
//!   - User scope, no extra entropy. Decrypt: base64 -> CryptUnprotectData.
//!
//! Non-Windows: `store_password` and `delete_password` are no-ops returning
//! Ok(()) so the dev box `save_credential` path still works end-to-end;
//! `resolve_password` returns `UecmError::PowerShell("DPAPI is Windows-only")`.

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
    let store = read_store()?;
    let encoded = store.get(alias).ok_or_else(|| {
        UecmError::OperationFailed(format!("no DPAPI entry for alias '{}'", alias))
    })?;
    let ciphertext = base64_decode(encoded)?;
    let plaintext = dpapi::unprotect(&ciphertext)?;
    String::from_utf8(plaintext).map_err(|e| {
        UecmError::OperationFailed(format!("DPAPI plaintext is not valid UTF-8: {}", e))
    })
}

#[cfg(not(windows))]
pub fn resolve_password(alias: &str) -> UecmResult<String> {
    let _ = alias;
    Err(UecmError::PowerShell(
        "DPAPI is Windows-only".to_string(),
    ))
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
// DPAPI FFI (Windows only)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod dpapi {
    use crate::error::{UecmError, UecmResult};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
    };
    use windows::Win32::Foundation::{HLOCAL, LocalFree};

    pub fn protect(plaintext: &[u8]) -> UecmResult<Vec<u8>> {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: plaintext.len() as u32,
            pbData: plaintext.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptProtectData(
                &mut input,
                None,
                None,
                None,
                None,
                0,
                &mut output,
            )
            .map_err(|e| {
                UecmError::OperationFailed(format!("CryptProtectData failed: {}", e))
            })?;
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
        };
        unsafe {
            let _ = LocalFree(HLOCAL(output.pbData as *mut _));
        }
        Ok(bytes)
    }

    pub fn unprotect(ciphertext: &[u8]) -> UecmResult<Vec<u8>> {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: ciphertext.len() as u32,
            pbData: ciphertext.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(
                &mut input,
                None,
                None,
                None,
                None,
                0,
                &mut output,
            )
            .map_err(|e| {
                UecmError::OperationFailed(format!("CryptUnprotectData failed: {}", e))
            })?;
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
        };
        unsafe {
            let _ = LocalFree(HLOCAL(output.pbData as *mut _));
        }
        Ok(bytes)
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

    #[cfg(not(windows))]
    #[test]
    fn resolve_password_returns_powershell_error_on_non_windows() {
        let result = resolve_password("UECM:winrm:HOST");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

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
}
