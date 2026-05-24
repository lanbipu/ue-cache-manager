use std::process::Command;

#[test]
fn invalid_flag_json_is_error_envelope_exit_64() {
    let exe = env!("CARGO_BIN_EXE_uecm-cli");
    let out = Command::new(exe)
        .args(["--no-such-flag", "--output", "json"])
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(64));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["status"], "error");
    assert_eq!(err["error"]["code"], "usage_error");
    assert_eq!(err["error"]["exit_code"], 64);
}
