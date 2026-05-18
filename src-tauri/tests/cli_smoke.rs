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

// -------------------------------------------------------------------------
// Plan 7 T2.5: M2 zen subcommands
// -------------------------------------------------------------------------

#[test]
fn zen_register_for_unknown_machine_returns_invalid_input() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args([
            "--json", "zen", "register",
            "--machine", "9999",
            "--role", "local",
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
fn zen_register_then_lua_preview_round_trip() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    // Seed a machine via `machine add` so the FK is satisfied. The CLI emits
    // a Completed event with the created id we can parse out.
    let added = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "machine", "add", "--ip", "192.168.10.30", "--hostname", "ZEN-01"])
        .output()
        .expect("spawn");
    assert!(added.status.success(), "stderr: {}", String::from_utf8_lossy(&added.stderr));

    // Register an endpoint with all defaults except role.
    let reg = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args([
            "--json", "zen", "register",
            "--machine", "1",
            "--role", "local",
            "--data-dir", "D:\\ZenData",
        ])
        .output()
        .expect("spawn");
    assert!(reg.status.success(), "stderr: {}", String::from_utf8_lossy(&reg.stderr));
    let reg_doc: serde_json::Value =
        serde_json::from_str(String::from_utf8(reg.stdout).unwrap().trim_end()).unwrap();
    assert_eq!(reg_doc["ok"], serde_json::Value::Bool(true));
    assert_eq!(reg_doc["inserted"], serde_json::Value::Bool(true));
    assert_eq!(reg_doc["declared_port"], serde_json::Value::from(8558));
    assert_eq!(reg_doc["role"], "local");
    assert_eq!(reg_doc["lifecycle_mode"], "editor_owned");
    let endpoint_id = reg_doc["endpoint_id"].as_i64().expect("endpoint_id");

    // Re-register the same (machine, port) returns inserted=false.
    let reg2 = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args([
            "--json", "zen", "register",
            "--machine", "1",
            "--role", "local",
            "--data-dir", "D:\\ZenData",
        ])
        .output()
        .expect("spawn");
    assert!(reg2.status.success());
    let reg2_doc: serde_json::Value =
        serde_json::from_str(String::from_utf8(reg2.stdout).unwrap().trim_end()).unwrap();
    assert_eq!(reg2_doc["inserted"], serde_json::Value::Bool(false));
    assert_eq!(reg2_doc["endpoint_id"], serde_json::Value::from(endpoint_id));

    // lua-preview renders the deterministic Lua text for the row.
    let preview = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args([
            "--json", "zen", "lua-preview",
            "--endpoint-id", &endpoint_id.to_string(),
        ])
        .output()
        .expect("spawn");
    assert!(preview.status.success(), "stderr: {}", String::from_utf8_lossy(&preview.stderr));
    let preview_doc: serde_json::Value =
        serde_json::from_str(String::from_utf8(preview.stdout).unwrap().trim_end()).unwrap();
    let lua = preview_doc["lua"].as_str().expect("lua string");
    assert!(lua.contains("server = {"));
    assert!(lua.contains("port = 8558"));
    assert!(lua.contains("datadir = \"D:\\\\ZenData\""));
}

#[test]
fn zen_unregister_without_yes_returns_invalid_input() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "zen", "unregister", "--endpoint-id", "1"])
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn zen_apply_config_dry_run_emits_plan_without_invoking_powershell() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    // Seed machine + endpoint.
    let _ = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "machine", "add", "--ip", "192.168.10.30", "--hostname", "ZEN-01"])
        .output()
        .expect("spawn");
    let reg = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args([
            "--json", "zen", "register",
            "--machine", "1",
            "--role", "local",
            "--data-dir", "D:\\ZenData",
        ])
        .output()
        .expect("spawn");
    let reg_doc: serde_json::Value =
        serde_json::from_str(String::from_utf8(reg.stdout).unwrap().trim_end()).unwrap();
    let endpoint_id = reg_doc["endpoint_id"].as_i64().unwrap();

    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args([
            "--json", "zen", "apply-config",
            "--endpoint-id", &endpoint_id.to_string(),
            "--dest-path", "C:\\Tools\\UECM\\zen.lua",
            "--dry-run",
        ])
        .output()
        .expect("spawn");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v["kind"], "completed");
    assert_eq!(v["summary"]["dry_run"], serde_json::Value::Bool(true));
    assert_eq!(v["summary"]["operation"], "zen.apply-config");
    assert!(v["summary"]["details"]["lua"].as_str().unwrap().contains("server = {"));
    assert_eq!(v["summary"]["details"]["dest_path"], "C:\\Tools\\UECM\\zen.lua");
}
