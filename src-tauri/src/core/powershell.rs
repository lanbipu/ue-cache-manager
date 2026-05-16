//! PowerShell sidecar invocation. On Windows, runs powershell.exe with
//! provided script + args, captures stdout, returns parsed JSON result.
//! On non-Windows, returns an error (sidecar is Windows-only).

use crate::error::{UecmError, UecmResult};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;

/// Decode bytes captured from a Windows subprocess (powershell.exe,
/// robocopy.exe, etc.).
///
/// PowerShell 5.x and other native Windows tools on Chinese Windows emit
/// stderr in the OEM/ANSI codepage (CP936 / GBK) rather than UTF-8, so
/// `from_utf8_lossy` produces a wall of U+FFFD replacement characters that
/// hides the real error. Try strict UTF-8 first (covers English systems and
/// PowerShell 7+ defaults), fall back to GBK on failure.
#[cfg(any(windows, test))]
pub(crate) fn decode_subprocess_output(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, had_errors) = encoding_rs::GBK.decode(bytes);
            if had_errors {
                tracing::warn!(
                    bytes_len = bytes.len(),
                    "powershell output is neither valid UTF-8 nor clean GBK; \
                     decoded string may contain U+FFFD"
                );
            }
            cow.into_owned()
        }
    }
}

/// Resolve a sidecar script name to its on-disk path.
/// Respects `UECM_PS_DIR` env override, then searches:
///   1. `<exe-dir>/ps-scripts/<name>` — production install (Tauri bundle.resources)
///   2. `<repo-root>/ps-scripts/<name>` — dev builds via `CARGO_MANIFEST_DIR`
pub fn script_path(name: &str) -> PathBuf {
    crate::startup::resolve_ps_script_dir().join(name)
}

/// Load a sidecar script's text. Used when the script body is forwarded over
/// WinRM via `core::winrm::invoke*` rather than executed locally.
pub fn read_script(name: &str) -> UecmResult<String> {
    Ok(fs::read_to_string(script_path(name))?)
}

/// Resolve a vendored binary's on-disk path (e.g. `PsExec64.exe`).
/// Respects `UECM_VENDOR_DIR` env override, then searches:
///   1. `<exe-dir>/vendor/<name>` — production install
///   2. `<repo-root>/vendor/<name>` — dev builds via `CARGO_MANIFEST_DIR`
pub fn vendor_path(name: &str) -> PathBuf {
    if let Ok(over) = std::env::var("UECM_VENDOR_DIR") {
        return PathBuf::from(over).join(name);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("vendor").join(name);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("vendor")
        .join(name)
}

#[derive(Debug)]
pub struct ScriptResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Run a .ps1 script with the given arguments. Returns raw output.
pub fn run_script(script_path: &Path, args: &[&str]) -> UecmResult<ScriptResult> {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("powershell.exe");
        cmd.arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(script_path);
        for arg in args {
            cmd.arg(arg);
        }
        let output = cmd.output().map_err(|e| {
            UecmError::PowerShell(format!("failed to spawn powershell.exe: {}", e))
        })?;
        Ok(ScriptResult {
            stdout: decode_subprocess_output(&output.stdout),
            stderr: decode_subprocess_output(&output.stderr),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (script_path, args);
        Err(UecmError::PowerShell(
            "PowerShell sidecar is Windows-only".to_string(),
        ))
    }
}

/// Run a script and parse stdout as JSON of type T.
///
/// Most sidecars emit `{ ok: bool, ... }` to stdout AND `exit 1` on the
/// catch path so callers that only check exit code see an empty stderr.
/// Try to parse stdout first regardless of exit code — if it deserializes
/// to T, return it (caller inspects the `ok` field). Only fall back to the
/// raw exit-code error message when stdout doesn't parse cleanly.
pub fn run_json<T: DeserializeOwned>(script_path: &Path, args: &[&str]) -> UecmResult<T> {
    let result = run_script(script_path, args)?;
    if !result.stdout.trim().is_empty() {
        if let Ok(parsed) = serde_json::from_str::<T>(&result.stdout) {
            return Ok(parsed);
        }
    }
    if result.exit_code != 0 {
        return Err(UecmError::PowerShell(format!(
            "script exited with code {}: {}",
            result.exit_code,
            if result.stderr.trim().is_empty() {
                result.stdout.trim()
            } else {
                result.stderr.trim()
            }
        )));
    }
    serde_json::from_str(&result.stdout).map_err(|e| {
        UecmError::PowerShell(format!(
            "failed to parse JSON output: {} (stdout: {})",
            e, result.stdout
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ENV_TEST_LOCK;
    #[cfg(windows)]
    use serde::Deserialize;

    #[cfg(windows)]
    #[derive(Debug, Deserialize)]
    struct EchoOutput {
        received: String,
        machine: String,
    }

    #[cfg(windows)]
    #[test]
    fn test_echo_script_returns_parsed_json() {
        let script = Path::new("../ps-scripts/test-echo.ps1");
        let result: EchoOutput = run_json(script, &["world"]).unwrap();
        assert_eq!(result.received, "world");
        assert!(!result.machine.is_empty());
    }

    #[cfg(not(windows))]
    #[test]
    fn run_script_returns_error_on_non_windows() {
        let script = Path::new("../ps-scripts/test-echo.ps1");
        let result = run_script(script, &[]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UecmError::PowerShell(_)));
    }

    #[test]
    fn decode_handles_pure_ascii() {
        let bytes = b"-File ..\\ps-scripts\\test-echo.ps1";
        assert_eq!(
            decode_subprocess_output(bytes),
            "-File ..\\ps-scripts\\test-echo.ps1"
        );
    }

    #[test]
    fn decode_handles_valid_utf8() {
        let bytes = "hello UTF-8 中文".as_bytes();
        assert_eq!(decode_subprocess_output(bytes), "hello UTF-8 中文");
    }

    #[test]
    fn decode_falls_back_to_gbk_for_chinese_windows_stderr() {
        // GBK bytes for "无法找到文件" — a fragment of PowerShell 5.x's
        // ScriptFileNotProvided message on zh-CN Windows. Decoder must
        // recover the original characters without producing U+FFFD.
        let gbk_bytes: &[u8] = &[
            0xCE, 0xDE, 0xB7, 0xA8, 0xD5, 0xD2, 0xB5, 0xBD, 0xCE, 0xC4, 0xBC, 0xFE,
        ];
        let decoded = decode_subprocess_output(gbk_bytes);
        assert_eq!(decoded, "无法找到文件");
        assert!(!decoded.contains('\u{FFFD}'));
    }

    #[test]
    fn script_path_respects_env_override() {
        let _lock = ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("UECM_PS_DIR", "/tmp/test-ps-override");
        let p = script_path("foo.ps1");
        assert_eq!(p, std::path::PathBuf::from("/tmp/test-ps-override/foo.ps1"));
        std::env::remove_var("UECM_PS_DIR");
    }
}
