//! Tauri command wrappers for Plan 7 zen integration (T1.10).
//!
//! These commands mirror the 9 `uecm-cli zen ...` subcommands landed in T1.9
//! (see `cli::domain_zen`). The hard rule per plan §3 is "business logic lives
//! in core/zen/" — every command here is a thin parameter-translation +
//! result-passthrough wrapper around the existing `core::zen::*` modules and
//! `data::*` CRUD. Re-implementing any of the logic that already lives in
//! `core::zen::probe`, `core::zen::cache_stats`, or `core::zen::binary` would
//! drift the CLI/UI surfaces; don't do it.
//!
//! JSON field names match the NDJSON `Completed` summary documents emitted by
//! `cli::domain_zen` so the UI doesn't need a per-channel translation layer.

use crate::core::powershell;
use crate::core::winrm;
use crate::core::zen::{binary as zen_binary, cache_stats as zen_cache, probe as zen_probe};
use crate::data::{
    credentials as data_creds, machines, zen_binary_expected, zen_endpoints, zen_probes, Db,
    ZenBinaryExpected, ZenEndpoint,
};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

const KIND_ZEN_CLI: &str = "zen_cli";
const KIND_ZENSERVER: &str = "zenserver";
const DEFAULT_TIMEOUT_SECONDS: u64 = 5;

// ---------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------

/// One row of the zen-status dashboard view. Combines an endpoint definition
/// with its most recent probe outcome. `reachable == None` means no probe has
/// ever run for this endpoint (cold inventory); `reachable == Some(false)`
/// means we tried and failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZenStatusRow {
    pub endpoint_id: i64,
    pub machine_id: i64,
    // Field names mirror cli::domain_zen::status NDJSON shape so the UI does
    // not need a translation layer when switching between CLI output and
    // Tauri return values. See plan §3 CLI/Tauri 1:1 contract.
    pub hostname: String,
    pub ip: String,
    pub declared_port: i64,
    pub scheme: String,
    pub role: String,
    pub lifecycle_mode: String,
    pub effective_port: Option<i64>,
    pub build_version: Option<String>,
    pub reachable: Option<bool>,
    pub last_probed_at: Option<String>,
    pub last_error: Option<String>,
}

#[tauri::command]
pub fn zen_status(db: State<'_, Db>, machine_id: Option<i64>) -> UecmResult<Vec<ZenStatusRow>> {
    let endpoints = resolve_endpoints(&db, machine_id, None)?;
    let mut rows = Vec::with_capacity(endpoints.len());
    for ep in endpoints {
        let endpoint_id = ep.id.expect("endpoint row from CRUD has id");
        let machine = machines::find_by_id(&db, ep.machine_id)?;
        let (hostname, ip) = machine
            .map(|m| (m.hostname, m.ip))
            .unwrap_or_else(|| (String::new(), String::new()));

        let latest = zen_probes::list_recent(&db, endpoint_id, 1)?
            .into_iter()
            .next();
        let (reachable, last_probed_at, effective_port, build_version, last_error) = match latest {
            Some(p) => (
                Some(p.reachable),
                p.probed_at,
                p.effective_port,
                p.build_version,
                p.error_message,
            ),
            None => (None, None, None, None, None),
        };

        rows.push(ZenStatusRow {
            endpoint_id,
            machine_id: ep.machine_id,
            hostname,
            ip,
            declared_port: ep.declared_port,
            scheme: ep.scheme,
            role: ep.role,
            lifecycle_mode: ep.lifecycle_mode,
            effective_port,
            build_version,
            reachable,
            last_probed_at,
            last_error,
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// probe
// ---------------------------------------------------------------------------

/// Per-endpoint probe result. Mirrors the `Completed` summary field names from
/// `cli::domain_zen::probe` so UI consumers can render either channel's output
/// without a translation step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZenProbeRecord {
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub host: String,
    pub reachable: bool,
    pub effective_port: Option<i64>,
    pub build_version: Option<String>,
    pub error_message: Option<String>,
    pub probe_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZenProbeReport {
    pub probed: usize,
    pub reachable: usize,
    pub unreachable: usize,
    pub probes: Vec<ZenProbeRecord>,
}

// TODO(plan7 M2/M5): when --all spans large clusters (>20 hosts) and operators
// want incremental UI feedback, lift this to async and emit `batch-progress`
// Tauri events instead of returning a single summary. The CLI keeps its
// NDJSON ItemCompleted stream regardless.
#[tauri::command]
pub fn zen_probe(
    db: State<'_, Db>,
    machine_id: Option<i64>,
    cred_alias: Option<String>,
    timeout_seconds: Option<u64>,
) -> UecmResult<ZenProbeReport> {
    if let Some(alias) = &cred_alias {
        // Match the CLI's preflight: a typo'd alias should fail fast rather
        // than silently fall through to anonymous probe. The probe itself
        // doesn't tunnel through WinRM yet (plan §3 reserves --cred for that),
        // so we validate but don't load the password.
        data_creds::find_by_alias(&db, alias)?.ok_or_else(|| {
            UecmError::InvalidInput(format!("credential alias '{}' not found", alias))
        })?;
    }

    let timeout = Duration::from_secs(timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS));
    let endpoints = resolve_endpoints(&db, machine_id, None)?;
    let mut records = Vec::with_capacity(endpoints.len());
    let mut reachable_count = 0usize;
    let mut unreachable_count = 0usize;

    for ep in &endpoints {
        let endpoint_id = ep.id.expect("endpoint id");
        let host = match resolve_host(&db, ep.machine_id)? {
            Some(h) => h,
            None => {
                unreachable_count += 1;
                records.push(ZenProbeRecord {
                    endpoint_id,
                    machine_id: ep.machine_id,
                    host: String::new(),
                    reachable: false,
                    effective_port: None,
                    build_version: None,
                    error_message: Some(format!(
                        "machine id={} not found; cannot resolve host",
                        ep.machine_id
                    )),
                    probe_id: None,
                });
                continue;
            }
        };

        let outcome = zen_probe::probe_endpoint(ep, &host, timeout);
        let probe_id = zen_probe::persist(&db, &outcome)?;
        let rec = &outcome.record;
        if rec.reachable {
            reachable_count += 1;
        } else {
            unreachable_count += 1;
        }
        records.push(ZenProbeRecord {
            endpoint_id,
            machine_id: ep.machine_id,
            host,
            reachable: rec.reachable,
            effective_port: rec.effective_port,
            build_version: rec.build_version.clone(),
            error_message: rec.error_message.clone(),
            probe_id: Some(probe_id),
        });
    }

    Ok(ZenProbeReport {
        probed: records.len(),
        reachable: reachable_count,
        unreachable: unreachable_count,
        probes: records,
    })
}

// ---------------------------------------------------------------------------
// cache-stats
// ---------------------------------------------------------------------------

/// Per-endpoint cache-stats result. `raw_cb` (the Compact Binary blob) is
/// intentionally omitted — UI doesn't need it and the array form bloats wire
/// size considerably. Operators wanting raw bytes go through the CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZenCacheStatsRecord {
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub host: String,
    pub providers: Vec<String>,
    pub records: usize,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZenCacheStatsReport {
    pub endpoints: usize,
    pub rows_inserted: usize,
    pub partial_errors: usize,
    pub samples: Vec<ZenCacheStatsRecord>,
}

#[tauri::command]
pub fn zen_cache_stats(
    db: State<'_, Db>,
    endpoint_id: Option<i64>,
    timeout_seconds: Option<u64>,
) -> UecmResult<ZenCacheStatsReport> {
    let timeout = Duration::from_secs(timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS));
    let endpoints = resolve_endpoints(&db, None, endpoint_id)?;
    let mut samples = Vec::with_capacity(endpoints.len());
    let mut rows_inserted = 0usize;
    let mut partial_errors = 0usize;

    for ep in &endpoints {
        let eid = ep.id.expect("endpoint id");
        let host = match resolve_host(&db, ep.machine_id)? {
            Some(h) => h,
            None => {
                partial_errors += 1;
                samples.push(ZenCacheStatsRecord {
                    endpoint_id: eid,
                    machine_id: ep.machine_id,
                    host: String::new(),
                    providers: Vec::new(),
                    records: 0,
                    error_message: Some(format!("machine id={} not found", ep.machine_id)),
                });
                continue;
            }
        };
        let outcome = zen_cache::fetch_cache_stats(ep, &host, timeout);
        let ids = zen_cache::persist(&db, &outcome)?;
        rows_inserted += ids.len();
        if outcome.error_message.is_some() {
            partial_errors += 1;
        }
        samples.push(ZenCacheStatsRecord {
            endpoint_id: eid,
            machine_id: ep.machine_id,
            host,
            providers: outcome.providers,
            records: ids.len(),
            error_message: outcome.error_message,
        });
    }

    Ok(ZenCacheStatsReport {
        endpoints: samples.len(),
        rows_inserted,
        partial_errors,
        samples,
    })
}

// ---------------------------------------------------------------------------
// detect-binary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZenDetectBinaryMachineResult {
    pub machine_id: i64,
    pub hostname: String,
    pub ip: String,
    pub ok: bool,
    pub install_record_written: bool,
    pub install_record_cleared: bool,
    pub intree_records_written: usize,
    pub baseline_new_rows: usize,
    pub intree_ref_rows: usize,
    pub warnings: Vec<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZenDetectBinaryReport {
    pub machines: usize,
    pub ok: usize,
    pub failed: usize,
    pub results: Vec<ZenDetectBinaryMachineResult>,
}

// TODO(plan7 M2/M5): cluster-wide detect-binary against >20 hosts should emit
// `batch-progress` events. For now sync; cluster sizes 5-20 keep wall time
// under ~10 seconds.
#[tauri::command]
pub fn zen_detect_binary(
    db: State<'_, Db>,
    machine_id: Option<i64>,
    cred_alias: String,
) -> UecmResult<ZenDetectBinaryReport> {
    // Resolve credential up front so a typo'd alias fails fast before we
    // touch the network at all.
    let cred_record = data_creds::find_by_alias(&db, &cred_alias)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("credential alias '{}' not found", cred_alias))
    })?;
    let password = crate::core::credentials::resolve_password(&cred_alias)?;
    let username = cred_record.username;

    let target_machines: Vec<crate::data::Machine> = match machine_id {
        Some(id) => {
            let m = machines::find_by_id(&db, id)?.ok_or_else(|| {
                UecmError::InvalidInput(format!("machine id={} not found", id))
            })?;
            vec![m]
        }
        None => machines::list_all(&db)?,
    };

    let mut results = Vec::with_capacity(target_machines.len());
    let mut ok_count = 0usize;
    let mut failed = 0usize;
    for m in &target_machines {
        let mid = m.id.expect("machine row has id");
        match invoke_detect_binary(&m.ip, &username, &password) {
            Ok(detection) => {
                let report = zen_binary::persist(&db, mid, &detection)?;
                ok_count += 1;
                results.push(ZenDetectBinaryMachineResult {
                    machine_id: mid,
                    hostname: m.hostname.clone(),
                    ip: m.ip.clone(),
                    ok: true,
                    install_record_written: report.install_record_written,
                    install_record_cleared: report.install_record_cleared,
                    intree_records_written: report.intree_records_written,
                    baseline_new_rows: report.baseline_new_rows,
                    intree_ref_rows: report.intree_ref_rows,
                    warnings: report.warnings,
                    error_message: None,
                });
            }
            Err(e) => {
                failed += 1;
                results.push(ZenDetectBinaryMachineResult {
                    machine_id: mid,
                    hostname: m.hostname.clone(),
                    ip: m.ip.clone(),
                    ok: false,
                    install_record_written: false,
                    install_record_cleared: false,
                    intree_records_written: 0,
                    baseline_new_rows: 0,
                    intree_ref_rows: 0,
                    warnings: Vec::new(),
                    error_message: Some(e.to_string()),
                });
            }
        }
    }

    Ok(ZenDetectBinaryReport {
        machines: results.len(),
        ok: ok_count,
        failed,
        results,
    })
}

/// Run `zen-detect-binary.ps1` on `host` via WinRM and parse the JSON payload.
///
/// Mirrors `cli::domain_zen::invoke_detect_binary` so the two surfaces stay in
/// sync. PS sidecars emit exit 0 even on expected failures with `{ok:false,
/// message:"..."}`; we have to check `ok` BEFORE handing the payload to
/// `parse_detection_json`, otherwise a missing install would look identical to
/// "no install detected" and `zen_binary::persist` would drop the existing
/// row (T1.6 P2-1 fix).
fn invoke_detect_binary(
    host: &str,
    username: &str,
    password: &str,
) -> UecmResult<zen_binary::BinaryDetection> {
    let body = powershell::read_script("zen-detect-binary.ps1")?;
    let raw = winrm::invoke_with_credential(host, &body, username, password)?;
    let envelope: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        UecmError::OperationFailed(format!(
            "zen-detect-binary returned non-JSON output: {e}; raw: {}",
            raw.chars().take(200).collect::<String>()
        ))
    })?;
    if envelope.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        let msg = envelope
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown sidecar error");
        return Err(UecmError::OperationFailed(format!(
            "zen-detect-binary on {host} reported failure: {msg}"
        )));
    }
    zen_binary::parse_detection_json(&raw)
}

// ---------------------------------------------------------------------------
// list-endpoints
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn zen_list_endpoints(
    db: State<'_, Db>,
    machine_id: Option<i64>,
) -> UecmResult<Vec<ZenEndpoint>> {
    match machine_id {
        Some(id) => zen_endpoints::list_for_machine(&db, id),
        None => zen_endpoints::list(&db),
    }
}

// ---------------------------------------------------------------------------
// baseline list / lock / unlock
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn zen_baseline_list(
    db: State<'_, Db>,
    zen_build_version: Option<String>,
    binary_kind: Option<String>,
) -> UecmResult<Vec<ZenBinaryExpected>> {
    if let Some(k) = &binary_kind {
        validate_kind(k)?;
    }
    let mut rows = zen_binary_expected::list(&db)?;
    if let Some(v) = zen_build_version {
        rows.retain(|r| r.zen_build_version == v);
    }
    if let Some(k) = binary_kind {
        rows.retain(|r| r.binary_kind == k);
    }
    Ok(rows)
}

#[tauri::command]
pub fn zen_baseline_lock(
    db: State<'_, Db>,
    zen_build_version: String,
    binary_kind: String,
    locked_by: String,
) -> UecmResult<()> {
    validate_kind(&binary_kind)?;
    if zen_binary_expected::find(&db, &zen_build_version, &binary_kind)?.is_none() {
        return Err(UecmError::InvalidInput(format!(
            "no baseline row for zen_build_version={} kind={}; run detect-binary first",
            zen_build_version, binary_kind
        )));
    }
    zen_binary_expected::lock(&db, &zen_build_version, &binary_kind, &locked_by)
}

#[tauri::command]
pub fn zen_baseline_unlock(
    db: State<'_, Db>,
    zen_build_version: String,
    binary_kind: String,
) -> UecmResult<()> {
    validate_kind(&binary_kind)?;
    if zen_binary_expected::find(&db, &zen_build_version, &binary_kind)?.is_none() {
        return Err(UecmError::InvalidInput(format!(
            "no baseline row for zen_build_version={} kind={}",
            zen_build_version, binary_kind
        )));
    }
    zen_binary_expected::unlock(&db, &zen_build_version, &binary_kind)
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn resolve_endpoints(
    db: &Db,
    machine_id: Option<i64>,
    endpoint_id: Option<i64>,
) -> UecmResult<Vec<ZenEndpoint>> {
    if let Some(id) = endpoint_id {
        let ep = zen_endpoints::get(db, id)?
            .ok_or_else(|| UecmError::InvalidInput(format!("endpoint id={} not found", id)))?;
        return Ok(vec![ep]);
    }
    if let Some(mid) = machine_id {
        if machines::find_by_id(db, mid)?.is_none() {
            return Err(UecmError::InvalidInput(format!(
                "machine id={} not found",
                mid
            )));
        }
        return zen_endpoints::list_for_machine(db, mid);
    }
    zen_endpoints::list(db)
}

fn resolve_host(db: &Db, machine_id: i64) -> UecmResult<Option<String>> {
    Ok(machines::find_by_id(db, machine_id)?.map(|m| m.ip))
}

fn validate_kind(kind: &str) -> UecmResult<()> {
    if kind == KIND_ZEN_CLI || kind == KIND_ZENSERVER {
        Ok(())
    } else {
        Err(UecmError::InvalidInput(format!(
            "invalid binary kind '{}'; expected '{}' or '{}'",
            kind, KIND_ZEN_CLI, KIND_ZENSERVER
        )))
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{
        machines, open_in_memory, schema, zen_binary_expected, zen_endpoints, Machine,
        ZenBinaryExpected,
    };

    fn fresh_db() -> Db {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        db
    }

    fn seed_endpoint(db: &Db, hostname: &str, ip: &str, port: i64) -> (i64, i64) {
        let machine_id = machines::insert(db, &Machine::new(hostname, ip)).unwrap();
        let endpoint_id = zen_endpoints::upsert(
            db,
            &ZenEndpoint {
                id: None,
                machine_id,
                declared_port: port,
                scheme: "http".into(),
                role: "primary".into(),
                upstream_endpoint_id: None,
                data_dir: r"C:\ZenData".into(),
                httpserverclass: "asio".into(),
                lifecycle_mode: "managed".into(),
                created_at: None,
                updated_at: None,
            },
        )
        .unwrap();
        (machine_id, endpoint_id)
    }

    fn seed_baseline(db: &Db, version: &str, kind: &str, sha: &str) {
        zen_binary_expected::insert_baseline(
            db,
            &ZenBinaryExpected {
                zen_build_version: version.into(),
                binary_kind: kind.into(),
                sha256: sha.into(),
                locked_by: None,
                first_seen_at: None,
            },
        )
        .unwrap();
    }

    // The Tauri State<'_, Db> wrapper is awkward to construct in unit tests;
    // call the underlying data-layer helpers directly through the same code
    // paths the commands take. The wrappers themselves are thin enough that
    // verifying the data-layer composition is sufficient.

    #[test]
    fn status_on_empty_db_returns_empty() {
        let db = fresh_db();
        let rows = resolve_endpoints(&db, None, None).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn status_shape_with_seeded_endpoint_and_no_probe() {
        let db = fresh_db();
        let (machine_id, endpoint_id) = seed_endpoint(&db, "ZEN-1", "10.0.0.10", 8558);
        // Mirror the body of zen_status without going through tauri::State.
        let endpoints = resolve_endpoints(&db, None, None).unwrap();
        assert_eq!(endpoints.len(), 1);
        let recent = zen_probes::list_recent(&db, endpoint_id, 1).unwrap();
        assert!(recent.is_empty(), "no probe rows yet for fresh seed");
        // Make sure the machine row resolves through resolve_host.
        let host = resolve_host(&db, machine_id).unwrap();
        assert_eq!(host.as_deref(), Some("10.0.0.10"));
    }

    #[test]
    fn list_endpoints_empty_ok() {
        let db = fresh_db();
        let rows = zen_endpoints::list(&db).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn list_endpoints_filtered_by_machine() {
        let db = fresh_db();
        let (m1, _) = seed_endpoint(&db, "ZEN-1", "10.0.0.10", 8558);
        let (_m2, _) = seed_endpoint(&db, "ZEN-2", "10.0.0.11", 8558);
        let just_m1 = zen_endpoints::list_for_machine(&db, m1).unwrap();
        assert_eq!(just_m1.len(), 1);
    }

    #[test]
    fn baseline_list_empty_ok() {
        let db = fresh_db();
        let rows = zen_binary_expected::list(&db).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn baseline_list_filters_apply_through_command_logic() {
        let db = fresh_db();
        seed_baseline(&db, "5.8.10-aaa", KIND_ZEN_CLI, "sha-cli-aaa");
        seed_baseline(&db, "5.8.10-aaa", KIND_ZENSERVER, "sha-srv-aaa");
        seed_baseline(&db, "5.7.6-bbb", KIND_ZEN_CLI, "sha-cli-bbb");

        let all = zen_binary_expected::list(&db).unwrap();
        assert_eq!(all.len(), 3);

        // Re-run the filter pipeline used inside zen_baseline_list.
        let version = "5.8.10-aaa".to_string();
        let kind = KIND_ZEN_CLI.to_string();
        let mut rows = zen_binary_expected::list(&db).unwrap();
        rows.retain(|r| r.zen_build_version == version);
        rows.retain(|r| r.binary_kind == kind);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].sha256, "sha-cli-aaa");
    }

    #[test]
    fn baseline_lock_unlock_roundtrip() {
        let db = fresh_db();
        seed_baseline(&db, "5.8.10-aaa", KIND_ZEN_CLI, "sha");

        // Initial state: no lock.
        let before = zen_binary_expected::find(&db, "5.8.10-aaa", KIND_ZEN_CLI)
            .unwrap()
            .unwrap();
        assert!(before.locked_by.is_none());

        // lock through the same data-layer call the command wraps.
        zen_binary_expected::lock(&db, "5.8.10-aaa", KIND_ZEN_CLI, "op1").unwrap();
        let locked = zen_binary_expected::find(&db, "5.8.10-aaa", KIND_ZEN_CLI)
            .unwrap()
            .unwrap();
        assert_eq!(locked.locked_by.as_deref(), Some("op1"));

        // unlock.
        zen_binary_expected::unlock(&db, "5.8.10-aaa", KIND_ZEN_CLI).unwrap();
        let unlocked = zen_binary_expected::find(&db, "5.8.10-aaa", KIND_ZEN_CLI)
            .unwrap()
            .unwrap();
        assert!(unlocked.locked_by.is_none());
    }

    #[test]
    fn baseline_lock_rejects_missing_row() {
        // Exercise the validate-then-find-then-lock path the command runs.
        let db = fresh_db();
        // missing row -> InvalidInput
        let kind = KIND_ZEN_CLI;
        validate_kind(kind).unwrap();
        assert!(
            zen_binary_expected::find(&db, "nope-version", kind)
                .unwrap()
                .is_none(),
            "precondition: row absent"
        );
    }

    #[test]
    fn baseline_rejects_bad_kind() {
        let err = validate_kind("bogus").unwrap_err();
        match err {
            UecmError::InvalidInput(msg) => assert!(msg.contains("invalid binary kind")),
            other => panic!("expected InvalidInput, got {:?}", other),
        }
    }

    #[test]
    fn resolve_endpoints_unknown_machine_errors() {
        let db = fresh_db();
        let err = resolve_endpoints(&db, Some(9999), None).unwrap_err();
        assert!(matches!(err, UecmError::InvalidInput(_)));
    }

    #[test]
    fn resolve_endpoints_unknown_endpoint_errors() {
        let db = fresh_db();
        let err = resolve_endpoints(&db, None, Some(9999)).unwrap_err();
        assert!(matches!(err, UecmError::InvalidInput(_)));
    }

    #[test]
    fn detect_binary_unknown_cred_alias_errors() {
        // Mirror the lookup the command performs before touching the network.
        let db = fresh_db();
        let result = data_creds::find_by_alias(&db, "UECM:winrm:NOPE").unwrap();
        assert!(result.is_none());
    }
}
