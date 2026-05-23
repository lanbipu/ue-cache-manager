//! Thin Rust wrapper over the WinRM PowerShell sidecar scripts.
//! All operations are Windows-only at runtime; non-Windows returns
//! `UecmError::PowerShell("WinRM is Windows-only")` so the codebase still builds + tests on dev machines.

use crate::core::{loopback, powershell};
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ProbeResult {
    pub ok: bool,
    pub message: String,
    pub latency_ms: i64,
}

/// Probe a single host's WinRM availability.
pub fn probe(host: &str) -> UecmResult<ProbeResult> {
    if loopback::is_loopback_target(host) {
        return Ok(ProbeResult {
            ok: true,
            message: "loopback target; WinRM bypassed".to_string(),
            latency_ms: 0,
        });
    }
    let script = powershell::script_path("test-winrm.ps1");
    powershell::run_json::<ProbeResult>(&script, &["-HostName", host])
}

/// Probe a single host's WinRM availability with explicit credentials.
///
/// `test-winrm.ps1` uses `Test-WSMan` which is an unauthenticated connectivity
/// check — it does not forward credentials to the remote endpoint. Passing
/// `user`/`pass` here therefore has no effect on the probe outcome; the
/// credential pair is accepted for API symmetry but intentionally ignored so
/// callers can treat probe as a uniform first-step check regardless of auth mode.
pub fn probe_with_credential(host: &str, user: &str, pass: &str) -> UecmResult<ProbeResult> {
    let _ = (user, pass);
    probe(host)
}

/// Invoke a PowerShell scriptblock on a remote host. Returns combined stdout.
/// The script body is passed via stdin (no escaping required).
#[cfg(windows)]
pub fn invoke(host: &str, script_body: &str) -> UecmResult<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    if loopback::is_loopback_target(host) {
        return invoke_local(script_body);
    }

    let wrapper = powershell::script_path("invoke-remote.ps1");

    let mut child = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&wrapper)
        .arg("-HostName")
        .arg(host)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| UecmError::PowerShell(format!("failed to spawn powershell.exe: {}", e)))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| UecmError::PowerShell("failed to open stdin".to_string()))?;
        stdin
            .write_all(script_body.as_bytes())
            .map_err(|e| UecmError::PowerShell(format!("failed to write stdin: {}", e)))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| UecmError::PowerShell(format!("wait failed: {}", e)))?;

    if !output.status.success() {
        let stderr = powershell::decode_subprocess_output(&output.stderr);
        return Err(UecmError::PowerShell(format!(
            "remote invoke failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr
        )));
    }

    Ok(powershell::decode_subprocess_output(&output.stdout))
}

#[cfg(not(windows))]
pub fn invoke(host: &str, script_body: &str) -> UecmResult<String> {
    let _ = (host, script_body);
    Err(UecmError::PowerShell(
        "WinRM is Windows-only".to_string(),
    ))
}

/// Invoke a remote script with explicit credentials. Same shape as `invoke`
/// but forwards `-Username`/`-Password` to the wrapper script so the remote
/// session authenticates as that user instead of inheriting the caller's
/// Kerberos/NTLM context.
#[cfg(windows)]
pub fn invoke_with_credential(
    host: &str,
    script_body: &str,
    username: &str,
    password: &str,
    auth_method: &str,
) -> UecmResult<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    if loopback::is_loopback_target(host) {
        let _ = (username, password, auth_method);
        return invoke_local(script_body);
    }

    let wrapper = powershell::script_path("invoke-remote.ps1");

    let mut child = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&wrapper)
        .arg("-HostName")
        .arg(host)
        .arg("-Username")
        .arg(username)
        .arg("-Password")
        .arg(password)
        .arg("-AuthMethod")
        .arg(auth_method)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| UecmError::PowerShell(format!("failed to spawn powershell.exe: {}", e)))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| UecmError::PowerShell("failed to open stdin".to_string()))?;
        stdin
            .write_all(script_body.as_bytes())
            .map_err(|e| UecmError::PowerShell(format!("failed to write stdin: {}", e)))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| UecmError::PowerShell(format!("wait failed: {}", e)))?;

    if !output.status.success() {
        let stderr = powershell::decode_subprocess_output(&output.stderr);
        return Err(UecmError::PowerShell(format!(
            "remote invoke failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr
        )));
    }

    Ok(powershell::decode_subprocess_output(&output.stdout))
}

#[cfg(not(windows))]
pub fn invoke_with_credential(
    host: &str,
    script_body: &str,
    username: &str,
    password: &str,
    auth_method: &str,
) -> UecmResult<String> {
    let _ = (host, script_body, username, password, auth_method);
    Err(UecmError::PowerShell(
        "WinRM is Windows-only".to_string(),
    ))
}

/// RAII guard that removes a path when dropped. Codex round-18 P2: the
/// previous `let _ = remove_file` after the `Command::output()` call only
/// fires on the success / clean-error path. If a panic / abort / OS-level
/// kill landed between file creation and the explicit remove (or if `?` on
/// the spawn-error short-circuited past it), the temp `.ps1` — which may
/// contain `-ServicePassword <secret>` or similar inline-passed secrets —
/// stayed on disk. A Drop guard guarantees cleanup on every unwind path.
#[cfg(windows)]
struct TempScriptGuard(std::path::PathBuf);

#[cfg(windows)]
impl Drop for TempScriptGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(windows)]
fn invoke_local(script_body: &str) -> UecmResult<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    // T1.11 fix: `powershell.exe -Command -` reads stdin as a command
    // string, but Windows PowerShell truncates / silently drops large
    // multi-statement scripts piped via stdin (zen-detect-binary.ps1
    // is ~10K of functions + try/catch — emits zero stdout on the
    // stdin path even though `-File <tempfile>` runs it cleanly).
    //
    // Workaround: write the script to a temp .ps1 file and run via
    // `-File <path>`. Same code path PowerShell uses internally for
    // its own script execution, no stdin-buffer quirks.
    //
    // Codex round-18 P2 note on the script body: callers like
    // `zen_service_install` substitute secrets (e.g. `-ServicePassword
    // <pwd>`) into `script_body` before reaching this function. The
    // resulting temp `.ps1` therefore CAN contain plaintext secrets for
    // the lifetime of the powershell.exe child process. The RAII guard
    // below guarantees the file is removed on every exit path; the file
    // path is per-process / per-nanosecond so it's not predictable, and
    // %TEMP% defaults to the current user's profile (ACL'd to that
    // user). For deeper hardening (e.g. moving secrets out of the
    // script body into stdin / env), we'd need to rework every sidecar
    // that consumes them — tracked as a Plan 8 follow-up.
    let mut tmp_path = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    tmp_path.push(format!("uecm-invoke-{pid}-{nanos}.ps1"));

    // Bind the cleanup guard BEFORE creating the file so even
    // `File::create` failure (which short-circuits via `?`) goes
    // through the guard's Drop — `remove_file` on a path that doesn't
    // exist is a no-op, so this is safe.
    let _guard = TempScriptGuard(tmp_path.clone());

    {
        let mut tmp = std::fs::File::create(&tmp_path).map_err(|e| {
            UecmError::PowerShell(format!(
                "failed to create temp script file {}: {}",
                tmp_path.display(),
                e
            ))
        })?;
        tmp.write_all(
            b"[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null\r\n",
        )
        .and_then(|_| tmp.write_all(script_body.as_bytes()))
        .map_err(|e| UecmError::PowerShell(format!("failed to write temp script: {}", e)))?;
        // `tmp` (the File handle) drops at this `}` — file is flushed
        // and closed before powershell.exe reads it via `-File`.
    }

    let result = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&tmp_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    // Guard drops at end of function (or on panic / `?` unwind) and
    // removes the temp file unconditionally.

    let output = result
        .map_err(|e| UecmError::PowerShell(format!("failed to spawn powershell.exe: {}", e)))?;

    if !output.status.success() {
        let stderr = powershell::decode_subprocess_output(&output.stderr);
        return Err(UecmError::PowerShell(format!(
            "local invoke failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr
        )));
    }

    Ok(powershell::decode_subprocess_output(&output.stdout))
}

/// Invoke a remote script and parse stdout as JSON of type T.
pub fn invoke_json<T: serde::de::DeserializeOwned>(host: &str, script_body: &str) -> UecmResult<T> {
    let raw = invoke(host, script_body)?;
    serde_json::from_str(&raw).map_err(|e| {
        UecmError::PowerShell(format!("failed to parse remote JSON: {} (raw: {})", e, raw))
    })
}

/// Invoke a remote script with explicit credentials and parse stdout as JSON.
pub fn invoke_json_with_credential<T: serde::de::DeserializeOwned>(
    host: &str,
    script_body: &str,
    username: &str,
    password: &str,
    auth_method: &str,
) -> UecmResult<T> {
    let raw = invoke_with_credential(host, script_body, username, password, auth_method)?;
    serde_json::from_str(&raw).map_err(|e| {
        UecmError::PowerShell(format!("failed to parse remote JSON: {} (raw: {})", e, raw))
    })
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn invoke_returns_error_on_non_windows() {
        let result = invoke("RENDER-01", "Get-Date");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UecmError::PowerShell(_)));
    }

    #[test]
    fn probe_returns_error_on_non_windows() {
        // probe goes through powershell::run_script which also fails on non-Windows
        let result = probe("RENDER-01");
        assert!(result.is_err());
    }

    #[test]
    fn invoke_with_credential_returns_error_on_non_windows() {
        let result = invoke_with_credential("RENDER-01", "Get-Date", "admin", "p@ss", "Negotiate");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UecmError::PowerShell(_)));
    }

    #[test]
    fn probe_with_credential_returns_error_on_non_windows() {
        let result = probe_with_credential("RENDER-01", "u", "p");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UecmError::PowerShell(_)));
    }
}
