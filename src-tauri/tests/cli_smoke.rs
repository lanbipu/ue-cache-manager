//! End-to-end smoke tests for `uecm-cli`. Spawns the compiled binary.
//! Cross-platform — no PowerShell required for the assertions here.

use std::process::Command;

fn bin() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push(if cfg!(debug_assertions) { "debug" } else { "release" });
    p.push(if cfg!(windows) { "uecm-cli.exe" } else { "uecm-cli" });
    p
}

#[test]
fn version_subcommand_works() {
    let out = Command::new(bin())
        .args(["--json", "system", "version"])
        .output()
        .expect("spawn uecm-cli");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v["binary"], "uecm-cli");
    assert!(v["version"].is_string());
}

#[test]
fn machine_list_on_fresh_db_returns_empty_array() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "machine", "list"])
        .output()
        .expect("spawn");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v, serde_json::Value::Array(vec![]));
}

#[test]
fn invalid_cidr_returns_invalid_input_exit_code() {
    let out = Command::new(bin())
        .args(["--json", "machine", "scan", "not-a-cidr"])
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2), "expected exit code 2 (invalid_input)");
}

#[test]
fn cred_list_on_fresh_db_returns_empty_array() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "cred", "list"])
        .output()
        .expect("spawn");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v, serde_json::Value::Array(vec![]));
}

#[test]
fn env_set_without_target_returns_invalid_input() {
    let out = Command::new(bin())
        .args(["--json", "env", "set", "--name", "X", "--value", "Y"])
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    // host_args.require_one() runs in-handler (clap group doesn't mark either
    // as required), so this is an InvalidInput (exit 2) not a clap usage error.
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8(out.stderr).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(stderr.trim_end()).expect("stderr should be JSON envelope");
    assert_eq!(v["code"], "invalid_input");
}

#[test]
fn env_set_does_not_leak_value_to_stderr() {
    // macOS will fail at PowerShell layer, but redaction must hold.
    let secret = "MY-VERY-SECRET-VALUE-DEF-456-NEVER-LEAK";
    let out = Command::new(bin())
        .args([
            "--json",
            "env",
            "set",
            "--host",
            "192.0.2.1",
            "--name",
            "X",
            "--value",
            secret,
        ])
        .output()
        .expect("spawn");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!combined.contains(secret), "value leaked: {}", combined);
}

#[test]
fn project_list_on_fresh_db_returns_empty_array() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "project", "list"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v, serde_json::Value::Array(vec![]));
}

#[test]
fn health_runs_on_fresh_db_returns_empty_array() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "health", "runs"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v, serde_json::Value::Array(vec![]));
}

#[test]
fn gpu_matrix_on_empty_db_returns_empty_matrix() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "gpu", "matrix"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    // Empty matrix has cells == []
    assert_eq!(v["cells"], serde_json::Value::Array(vec![]));
}

#[test]
fn ini_runs_on_fresh_db_returns_empty_array() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "ini", "runs"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v, serde_json::Value::Array(vec![]));
}

#[test]
fn machine_refresh_accepts_cred_alias_flag_without_clap_error() {
    // Pure arg-parse check: clap should accept --cred-alias on machine refresh
    // now (Plan 3 wiring). The command will fail at the DB lookup since the
    // machine doesn't exist, but clap must NOT reject the flag.
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args([
            "--json", "machine", "refresh", "999",
            "--cred-alias", "winrm-admin",
        ])
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    // Spec §4: --json errors are emitted as JSON envelopes to stderr.
    // clap usage errors exit 64; runtime invalid_input exits 2.
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected invalid_input (exit 2), got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(stderr.trim_end()).expect("stderr should be JSON envelope");
    assert_eq!(v["kind"], "error");
    assert_eq!(v["code"], "invalid_input");
}

// -------------------------------------------------------------------------
// Plan 7 T1.9: zen domain smoke tests
// -------------------------------------------------------------------------

#[test]
fn zen_status_on_empty_db_returns_empty_endpoints_doc() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "zen", "status"])
        .output()
        .expect("spawn");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v["endpoints"], serde_json::Value::Array(vec![]));
}

#[test]
fn zen_list_endpoints_on_empty_db_returns_empty_array() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "zen", "list-endpoints"])
        .output()
        .expect("spawn");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v, serde_json::Value::Array(vec![]));
}

#[test]
fn zen_baseline_list_on_empty_db_returns_empty_array() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "zen", "baseline", "list"])
        .output()
        .expect("spawn");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v, serde_json::Value::Array(vec![]));
}

#[test]
fn zen_baseline_lock_without_yes_returns_invalid_input() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args([
            "--json", "zen", "baseline", "lock",
            "--zen-build-version", "5.8.10-aaa",
            "--kind", "zen_cli",
            "--locked-by", "operator-1",
        ])
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8(out.stderr).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(stderr.trim_end()).expect("stderr JSON envelope");
    assert_eq!(v["code"], "invalid_input");
}

#[test]
fn zen_baseline_lock_rejects_bad_kind() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args([
            "--json", "zen", "baseline", "lock",
            "--zen-build-version", "5.8.10-aaa",
            "--kind", "bogus",
            "--locked-by", "operator-1",
            "--yes",
        ])
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
}
