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
