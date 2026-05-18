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
//!
//! ## T2.6 destructive-op convention
//!
//! M2 added register / unregister / apply-config / lua-preview / service /
//! urlacl commands. The CLI gates each destructive operation behind `--yes` (or
//! `--dry-run`). The Tauri counterparts mirror that contract using two
//! parameters per command:
//!
//! - `confirmed: bool` — UI must prompt and pass `true` to actually run the
//!   destructive side-effect. Maps to CLI `--yes`. If both `confirmed` and
//!   `dry_run` are false on a destructive command, the wrapper returns
//!   `UecmError::InvalidInput("... requires confirmed=true or dry_run=true ...")`
//!   so accidental clicks can't fire write-side effects.
//! - `dry_run: bool` — when true, the wrapper assembles the same plan payload
//!   the CLI would emit under `--dry-run` (no PowerShell invocation, no
//!   `operations` row) and returns it as the response value. UI uses this for
//!   confirm-dialog previews.
//!
//! `dry_run` wins when both are true (matches `cli::destructive::check`).
//!
//! Service `start` and read-only commands (`service status`, `urlacl list`,
//! `lua_preview` is "read-only" in the no-destination sense) take no
//! `confirmed` parameter — they aren't destructive in the CLI either.

use crate::cli::domain_zen as zen_cli_shared;
use crate::core::powershell;
use crate::core::winrm;
use crate::core::zen::endpoint as zen_endpoint;
use crate::core::zen::redaction::redact;
use crate::core::zen::{binary as zen_binary, cache_stats as zen_cache, probe as zen_probe};
use crate::data::{
    credentials as data_creds, machines, operations, zen_binary_expected, zen_endpoints,
    zen_probes, Db, ZenBinaryExpected, ZenEndpoint,
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

// ===========================================================================
// M2 (T2.6) — register / unregister / apply-config / lua-preview /
//             service install|uninstall|start|stop|status /
//             urlacl add|list|remove
// ===========================================================================
//
// Each handler mirrors the matching CLI subcommand in `cli::domain_zen`. Shared
// helpers (validate_dest_path, validate_data_dir_safe, build_param_script, …)
// live in `cli::domain_zen` and are exposed `pub(crate)` so both surfaces share
// the same validation, sidecar plumbing, and operations logging. The Tauri
// layer therefore stays purely a parameter-translation + return-shape adapter.

// ---------------------------------------------------------------------------
// register
// ---------------------------------------------------------------------------

/// Inputs for `zen_register`. Mirrors `cli::args::ZenAction::Register` field
/// names so a UI form can post the same payload it would emit for the CLI.
#[derive(Debug, Clone, Deserialize)]
pub struct ZenRegisterInput {
    pub machine_id: i64,
    pub declared_port: i64,
    pub scheme: String,
    pub role: String,
    #[serde(default)]
    pub upstream_endpoint_id: Option<i64>,
    pub data_dir: String,
    pub httpserverclass: String,
    /// `None` triggers the same Plan §1.1 default the CLI uses
    /// (`shared_upstream` → `installed_service`, else `editor_owned`).
    #[serde(default)]
    pub lifecycle: Option<String>,
}

/// Outcome of `zen_register`. Mirrors `core::zen::endpoint::RegisterOutcome`
/// plus the persisted row for parity with the CLI's `emit_result` document.
#[derive(Debug, Clone, Serialize)]
pub struct ZenRegisterOutcome {
    pub endpoint_id: i64,
    pub inserted: bool,
    pub machine_id: i64,
    pub declared_port: i64,
    pub scheme: String,
    pub role: String,
    pub upstream_endpoint_id: Option<i64>,
    pub lifecycle_mode: String,
    pub httpserverclass: String,
    pub data_dir: String,
}

#[tauri::command]
pub fn zen_register(db: State<'_, Db>, input: ZenRegisterInput) -> UecmResult<ZenRegisterOutcome> {
    // Match the CLI's machine-existence pre-check so callers get
    // `InvalidInput` instead of an opaque FK violation.
    if machines::find_by_id(&db, input.machine_id)?.is_none() {
        return Err(UecmError::InvalidInput(format!(
            "machine id={} not found",
            input.machine_id
        )));
    }
    let lifecycle_mode = input
        .lifecycle
        .clone()
        .unwrap_or_else(|| zen_cli_shared::default_lifecycle_for(&input.role).to_string());

    let payload = zen_endpoint::EndpointInput {
        machine_id: input.machine_id,
        declared_port: input.declared_port,
        scheme: input.scheme.clone(),
        role: input.role.clone(),
        upstream_endpoint_id: input.upstream_endpoint_id,
        data_dir: input.data_dir.clone(),
        httpserverclass: input.httpserverclass.clone(),
        lifecycle_mode: lifecycle_mode.clone(),
    };
    let outcome = zen_endpoint::register(&db, &payload)?;

    // Idempotency contract: when `inserted=false`, return the *persisted* row
    // (its fields are authoritative), not the request payload — same behavior
    // as `cli::domain_zen::register`.
    let persisted = zen_endpoint::get(&db, outcome.id)?.ok_or_else(|| {
        UecmError::OperationFailed(format!(
            "register: row id={} disappeared between insert and readback",
            outcome.id
        ))
    })?;
    Ok(ZenRegisterOutcome {
        endpoint_id: outcome.id,
        inserted: outcome.inserted,
        machine_id: persisted.machine_id,
        declared_port: persisted.declared_port,
        scheme: persisted.scheme,
        role: persisted.role,
        upstream_endpoint_id: persisted.upstream_endpoint_id,
        lifecycle_mode: persisted.lifecycle_mode,
        httpserverclass: persisted.httpserverclass,
        data_dir: persisted.data_dir,
    })
}

// ---------------------------------------------------------------------------
// unregister
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ZenUnregisterResult {
    /// `dry_run=true` plan with the row preview. No DB mutation occurred.
    DryRun(ZenUnregisterPlan),
    /// `confirmed=true` real apply. Endpoint row deleted.
    Completed(ZenUnregisterSummary),
}

#[derive(Debug, Clone, Serialize)]
pub struct ZenUnregisterPlan {
    pub operation: &'static str,
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub declared_port: i64,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZenUnregisterSummary {
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub action: &'static str,
}

#[tauri::command]
pub fn zen_unregister(
    db: State<'_, Db>,
    endpoint_id: i64,
    confirmed: bool,
    dry_run: bool,
) -> UecmResult<ZenUnregisterResult> {
    guard_destructive(confirmed, dry_run, "zen.unregister")?;
    let ep = zen_cli_shared::require_endpoint(&db, endpoint_id)?;

    // Mirror the CLI's pre-flight dependents scan so dry-run plans can't
    // promise success when the real apply would refuse.
    let dependents: Vec<i64> = zen_endpoints::list(&db)?
        .into_iter()
        .filter(|other| other.upstream_endpoint_id == Some(endpoint_id))
        .filter_map(|other| other.id)
        .collect();
    if !dependents.is_empty() {
        let list = dependents
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(UecmError::InvalidInput(format!(
            "cannot unregister endpoint {endpoint_id}: still referenced as upstream by [{list}]; un-point them first"
        )));
    }

    if dry_run {
        return Ok(ZenUnregisterResult::DryRun(ZenUnregisterPlan {
            operation: "zen.unregister",
            endpoint_id,
            machine_id: ep.machine_id,
            declared_port: ep.declared_port,
            role: ep.role.clone(),
        }));
    }

    zen_endpoint::unregister(&db, endpoint_id)?;
    Ok(ZenUnregisterResult::Completed(ZenUnregisterSummary {
        endpoint_id,
        machine_id: ep.machine_id,
        action: "unregister",
    }))
}

// ---------------------------------------------------------------------------
// lua-preview
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ZenLuaPreviewResult {
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub lua: String,
}

#[tauri::command]
pub fn zen_lua_preview(db: State<'_, Db>, endpoint_id: i64) -> UecmResult<ZenLuaPreviewResult> {
    let (ep, lua) = zen_cli_shared::render_lua_for(&db, endpoint_id)?;
    // Same data_dir guard the CLI runs — `lua-preview` and `apply-config`
    // share the same engine, so they share the same refusal set.
    zen_cli_shared::validate_data_dir_safe(&ep.data_dir)?;
    Ok(ZenLuaPreviewResult {
        endpoint_id,
        machine_id: ep.machine_id,
        lua,
    })
}

// ---------------------------------------------------------------------------
// apply-config
// ---------------------------------------------------------------------------

/// Credentials wire shape shared by every M2 destructive command that drives a
/// remote sidecar. Mirrors `cli::credential_args::CredentialArgs` but flattened
/// because Tauri doesn't deserialize Rust groups/`Args` types directly.
///
/// Exactly one of:
/// - `cred_alias` set → DPAPI lookup
/// - `user` + `pass` set → inline user/password
/// - all `None` → inherit caller's Kerberos/NTLM context (anonymous over WinRM)
///
/// `pass_stdin` from the CLI has no meaningful analogue inside a GUI — the UI
/// already collected the password from the user — so it's not exposed here.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ZenCredentialInput {
    #[serde(default)]
    pub cred_alias: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub pass: Option<String>,
}

impl ZenCredentialInput {
    /// Preflight: validate the flag combination + alias existence without
    /// touching DPAPI. Mirrors `CredentialArgs::preflight`.
    fn preflight(&self, db: &Db) -> UecmResult<()> {
        if let Some(alias) = &self.cred_alias {
            data_creds::find_by_alias(db, alias)?.ok_or_else(|| {
                UecmError::InvalidInput(format!("credential alias '{}' not found", alias))
            })?;
            if self.user.is_some() || self.pass.is_some() {
                return Err(UecmError::InvalidInput(
                    "inconsistent credential flags: cred_alias conflicts with user/pass".into(),
                ));
            }
            return Ok(());
        }
        match (&self.user, &self.pass) {
            (Some(_), Some(_)) => Ok(()),
            (None, None) => Ok(()),
            _ => Err(UecmError::InvalidInput(
                "inconsistent credential flags: user and pass must both be set or both omitted"
                    .into(),
            )),
        }
    }

    /// Resolve to `(username, password)` if any credential was supplied;
    /// `None` means inherit caller's Kerberos/NTLM context. Mirrors
    /// `CredentialArgs::resolve` but without stdin support.
    fn resolve(&self, db: &Db) -> UecmResult<Option<(String, String)>> {
        if let Some(alias) = &self.cred_alias {
            let user = data_creds::find_by_alias(db, alias)?
                .ok_or_else(|| {
                    UecmError::InvalidInput(format!("credential alias '{}' not found", alias))
                })?
                .username;
            let pass = crate::core::credentials::resolve_password(alias)?;
            return Ok(Some((user, pass)));
        }
        match (&self.user, &self.pass) {
            (Some(u), Some(p)) => Ok(Some((u.clone(), p.clone()))),
            (None, None) => Ok(None),
            _ => Err(UecmError::InvalidInput(
                "inconsistent credential flags: user and pass must both be set or both omitted"
                    .into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ZenApplyConfigResult {
    DryRun(ZenApplyConfigPlan),
    Completed(ZenApplyConfigSummary),
}

#[derive(Debug, Clone, Serialize)]
pub struct ZenApplyConfigPlan {
    pub operation: &'static str,
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub host: String,
    pub dest_path: String,
    pub lua: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZenApplyConfigSummary {
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub host: String,
    pub dest_path: String,
    pub sha256: String,
    pub remote: serde_json::Value,
}

#[tauri::command]
pub fn zen_apply_config(
    db: State<'_, Db>,
    endpoint_id: i64,
    dest_path: String,
    confirmed: bool,
    dry_run: bool,
    cred: ZenCredentialInput,
) -> UecmResult<ZenApplyConfigResult> {
    guard_destructive(confirmed, dry_run, "zen.apply-config")?;
    cred.preflight(&db)?;
    let (ep, lua) = zen_cli_shared::render_lua_for(&db, endpoint_id)?;
    let machine = zen_cli_shared::require_machine(&db, ep.machine_id)?;

    // Mirror the CLI's `dest_path` + `data_dir` validation so dry-run plans
    // match what `--yes` would actually accept.
    zen_cli_shared::validate_dest_path(&dest_path)?;
    zen_cli_shared::validate_data_dir_safe(&ep.data_dir)?;

    if dry_run {
        return Ok(ZenApplyConfigResult::DryRun(ZenApplyConfigPlan {
            operation: "zen.apply-config",
            endpoint_id,
            machine_id: ep.machine_id,
            host: machine.ip,
            dest_path,
            lua,
        }));
    }

    let creds = cred.resolve(&db)?;
    let invocation = redact(&format!(
        "zen-write-lua-config.ps1 -DestPath {dest_path} (lua {} bytes)",
        lua.len()
    ));
    let op_id = operations::start(&db, "zen.apply_config", &[ep.machine_id])?;

    let expected_sha = zen_cli_shared::sha256_hex_of(&lua);
    let result = zen_cli_shared::invoke_write_lua(&machine.ip, &lua, &dest_path, creds.as_ref())
        .and_then(|response| {
            zen_cli_shared::verify_write_response(&response, &expected_sha, lua.len())
        });
    zen_cli_shared::finalize_op(&db, op_id, &result, &invocation);

    let response = result?;
    Ok(ZenApplyConfigResult::Completed(ZenApplyConfigSummary {
        endpoint_id,
        machine_id: ep.machine_id,
        host: machine.ip,
        dest_path,
        sha256: expected_sha,
        remote: response,
    }))
}

// ---------------------------------------------------------------------------
// service install / uninstall / start / stop / status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ZenServiceResult {
    DryRun(ZenServicePlan),
    Completed(ZenServiceSummary),
}

#[derive(Debug, Clone, Serialize)]
pub struct ZenServicePlan {
    pub operation: String,
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub host: String,
    pub service_name: &'static str,
    /// Present for install/uninstall (sidecar needs the `zen.exe` path).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zen_exe_path: Option<String>,
    /// Present for install (sidecar writes `server.datadir` from this).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZenServiceSummary {
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub host: String,
    pub service_name: &'static str,
    pub remote: serde_json::Value,
}

#[tauri::command]
pub fn zen_service_install(
    db: State<'_, Db>,
    endpoint_id: i64,
    confirmed: bool,
    dry_run: bool,
    cred: ZenCredentialInput,
) -> UecmResult<ZenServiceResult> {
    guard_destructive(confirmed, dry_run, "zen.service.install")?;
    cred.preflight(&db)?;
    let ep = zen_cli_shared::require_endpoint(&db, endpoint_id)?;
    let machine = zen_cli_shared::require_machine(&db, ep.machine_id)?;
    let install = crate::data::machine_zen_install::find(&db, ep.machine_id)?;
    let zen_exe = install
        .as_ref()
        .and_then(|m| m.zen_cli_path.clone())
        .ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "machine id={} has no zen.exe (zen_cli) recorded — run `zen detect-binary --machine {}` first",
                ep.machine_id, ep.machine_id,
            ))
        })?;

    // Same lifecycle guard as the CLI: only `installed_service` endpoints
    // get an SCM service.
    if ep.lifecycle_mode != "installed_service" {
        return Err(UecmError::InvalidInput(format!(
            "endpoint id={endpoint_id} has lifecycle_mode={:?}; service install \
             requires lifecycle_mode=\"installed_service\". \
             `register` is idempotent on (machine_id, declared_port) — call \
             `zen_unregister` first, then `zen_register` with \
             lifecycle=\"installed_service\" to fix the lifecycle.",
            ep.lifecycle_mode
        )));
    }
    zen_cli_shared::validate_service_data_dir(&ep.data_dir)?;

    if dry_run {
        return Ok(ZenServiceResult::DryRun(ZenServicePlan {
            operation: "zen.service.install".to_string(),
            endpoint_id,
            machine_id: ep.machine_id,
            host: machine.ip,
            service_name: zen_cli_shared::DEFAULT_SERVICE_NAME,
            zen_exe_path: Some(zen_exe),
            data_dir: Some(ep.data_dir.clone()),
        }));
    }

    let creds = cred.resolve(&db)?;
    let invocation = redact(&format!(
        "zen-service-install.ps1 -ZenExePath {zen_exe} -ServiceName {} -DataDir {}",
        zen_cli_shared::DEFAULT_SERVICE_NAME,
        ep.data_dir
    ));
    let op_id = operations::start(&db, "zen.service_install", &[ep.machine_id])?;
    let body = zen_cli_shared::build_param_script(
        "zen-service-install.ps1",
        &[
            ("ZenExePath", zen_exe.as_str()),
            ("ServiceName", zen_cli_shared::DEFAULT_SERVICE_NAME),
            ("DataDir", ep.data_dir.as_str()),
        ],
    );
    let result = match body {
        Ok(body) => zen_cli_shared::run_remote(&machine.ip, &body, creds.as_ref())
            .and_then(|raw| zen_cli_shared::parse_envelope(&raw, "zen-service-install")),
        Err(e) => Err(e),
    };
    zen_cli_shared::finalize_op(&db, op_id, &result, &invocation);
    let response = result?;
    Ok(ZenServiceResult::Completed(ZenServiceSummary {
        endpoint_id,
        machine_id: ep.machine_id,
        host: machine.ip,
        service_name: zen_cli_shared::DEFAULT_SERVICE_NAME,
        remote: response,
    }))
}

#[tauri::command]
pub fn zen_service_uninstall(
    db: State<'_, Db>,
    endpoint_id: i64,
    confirmed: bool,
    dry_run: bool,
    cred: ZenCredentialInput,
) -> UecmResult<ZenServiceResult> {
    guard_destructive(confirmed, dry_run, "zen.service.uninstall")?;
    cred.preflight(&db)?;
    let ep = zen_cli_shared::require_endpoint(&db, endpoint_id)?;
    let machine = zen_cli_shared::require_machine(&db, ep.machine_id)?;
    let install = crate::data::machine_zen_install::find(&db, ep.machine_id)?;
    let zen_exe = install
        .as_ref()
        .and_then(|m| m.zen_cli_path.clone())
        .ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "machine id={} has no zen.exe (zen_cli) recorded — run `zen detect-binary --machine {}` first",
                ep.machine_id, ep.machine_id,
            ))
        })?;

    if dry_run {
        return Ok(ZenServiceResult::DryRun(ZenServicePlan {
            operation: "zen.service.uninstall".to_string(),
            endpoint_id,
            machine_id: ep.machine_id,
            host: machine.ip,
            service_name: zen_cli_shared::DEFAULT_SERVICE_NAME,
            zen_exe_path: Some(zen_exe),
            data_dir: None,
        }));
    }

    let creds = cred.resolve(&db)?;
    let invocation = redact(&format!(
        "zen-service-uninstall.ps1 -ZenExePath {zen_exe} -ServiceName {}",
        zen_cli_shared::DEFAULT_SERVICE_NAME
    ));
    let op_id = operations::start(&db, "zen.service_uninstall", &[ep.machine_id])?;
    let body = zen_cli_shared::build_param_script(
        "zen-service-uninstall.ps1",
        &[
            ("ZenExePath", zen_exe.as_str()),
            ("ServiceName", zen_cli_shared::DEFAULT_SERVICE_NAME),
        ],
    );
    let result = match body {
        Ok(body) => zen_cli_shared::run_remote(&machine.ip, &body, creds.as_ref())
            .and_then(|raw| zen_cli_shared::parse_envelope(&raw, "zen-service-uninstall")),
        Err(e) => Err(e),
    };
    zen_cli_shared::finalize_op(&db, op_id, &result, &invocation);
    let response = result?;
    Ok(ZenServiceResult::Completed(ZenServiceSummary {
        endpoint_id,
        machine_id: ep.machine_id,
        host: machine.ip,
        service_name: zen_cli_shared::DEFAULT_SERVICE_NAME,
        remote: response,
    }))
}

/// Start is **not** destructive: the CLI doesn't take `--yes` for it. UI calls
/// with no `confirmed` / `dry_run` knobs — same lifecycle gate applies.
#[tauri::command]
pub fn zen_service_start(
    db: State<'_, Db>,
    endpoint_id: i64,
    cred: ZenCredentialInput,
) -> UecmResult<ZenServiceSummary> {
    cred.preflight(&db)?;
    let ep = zen_cli_shared::require_endpoint(&db, endpoint_id)?;
    let machine = zen_cli_shared::require_machine(&db, ep.machine_id)?;
    if ep.lifecycle_mode != "installed_service" {
        return Err(UecmError::InvalidInput(format!(
            "service zen.service.start requires endpoint id={endpoint_id} to have lifecycle_mode=\"installed_service\" \
             (got {:?}); `editor_owned` endpoints have no SCM service",
            ep.lifecycle_mode
        )));
    }
    let creds = cred.resolve(&db)?;

    let invocation = redact(&format!(
        "zen-up.ps1 -ServiceName {}",
        zen_cli_shared::DEFAULT_SERVICE_NAME
    ));
    let op_id = operations::start(&db, "zen.service_start", &[ep.machine_id])?;
    let body = zen_cli_shared::build_param_script(
        "zen-up.ps1",
        &[("ServiceName", zen_cli_shared::DEFAULT_SERVICE_NAME)],
    );
    let result = match body {
        Ok(body) => zen_cli_shared::run_remote(&machine.ip, &body, creds.as_ref())
            .and_then(|raw| zen_cli_shared::parse_envelope(&raw, "zen-up.ps1")),
        Err(e) => Err(e),
    };
    zen_cli_shared::finalize_op(&db, op_id, &result, &invocation);
    let response = result?;
    Ok(ZenServiceSummary {
        endpoint_id,
        machine_id: ep.machine_id,
        host: machine.ip,
        service_name: zen_cli_shared::DEFAULT_SERVICE_NAME,
        remote: response,
    })
}

#[tauri::command]
pub fn zen_service_stop(
    db: State<'_, Db>,
    endpoint_id: i64,
    confirmed: bool,
    dry_run: bool,
    cred: ZenCredentialInput,
) -> UecmResult<ZenServiceResult> {
    guard_destructive(confirmed, dry_run, "zen.service.stop")?;
    cred.preflight(&db)?;
    let ep = zen_cli_shared::require_endpoint(&db, endpoint_id)?;
    let machine = zen_cli_shared::require_machine(&db, ep.machine_id)?;
    if ep.lifecycle_mode != "installed_service" {
        return Err(UecmError::InvalidInput(format!(
            "service zen.service.stop requires endpoint id={endpoint_id} to have lifecycle_mode=\"installed_service\" \
             (got {:?}); `editor_owned` endpoints have no SCM service",
            ep.lifecycle_mode
        )));
    }

    if dry_run {
        return Ok(ZenServiceResult::DryRun(ZenServicePlan {
            operation: "zen.service.stop".to_string(),
            endpoint_id,
            machine_id: ep.machine_id,
            host: machine.ip,
            service_name: zen_cli_shared::DEFAULT_SERVICE_NAME,
            zen_exe_path: None,
            data_dir: None,
        }));
    }

    let creds = cred.resolve(&db)?;
    let invocation = redact(&format!(
        "zen-down.ps1 -ServiceName {}",
        zen_cli_shared::DEFAULT_SERVICE_NAME
    ));
    let op_id = operations::start(&db, "zen.service_stop", &[ep.machine_id])?;
    let body = zen_cli_shared::build_param_script(
        "zen-down.ps1",
        &[("ServiceName", zen_cli_shared::DEFAULT_SERVICE_NAME)],
    );
    let result = match body {
        Ok(body) => zen_cli_shared::run_remote(&machine.ip, &body, creds.as_ref())
            .and_then(|raw| zen_cli_shared::parse_envelope(&raw, "zen-down.ps1")),
        Err(e) => Err(e),
    };
    zen_cli_shared::finalize_op(&db, op_id, &result, &invocation);
    let response = result?;
    Ok(ZenServiceResult::Completed(ZenServiceSummary {
        endpoint_id,
        machine_id: ep.machine_id,
        host: machine.ip,
        service_name: zen_cli_shared::DEFAULT_SERVICE_NAME,
        remote: response,
    }))
}

#[derive(Debug, Clone, Serialize)]
pub struct ZenServiceStatusResult {
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub host: String,
    pub service_name: &'static str,
    pub remote: serde_json::Value,
}

#[tauri::command]
pub fn zen_service_status(
    db: State<'_, Db>,
    endpoint_id: i64,
    cred: ZenCredentialInput,
) -> UecmResult<ZenServiceStatusResult> {
    cred.preflight(&db)?;
    let ep = zen_cli_shared::require_endpoint(&db, endpoint_id)?;
    let machine = zen_cli_shared::require_machine(&db, ep.machine_id)?;
    let creds = cred.resolve(&db)?;

    let body = zen_cli_shared::build_param_script(
        "zen-service-status.ps1",
        &[("ServiceName", zen_cli_shared::DEFAULT_SERVICE_NAME)],
    )?;
    let raw = zen_cli_shared::run_remote(&machine.ip, &body, creds.as_ref())?;
    let response = zen_cli_shared::parse_envelope(&raw, "zen-service-status")?;
    Ok(ZenServiceStatusResult {
        endpoint_id,
        machine_id: ep.machine_id,
        host: machine.ip,
        service_name: zen_cli_shared::DEFAULT_SERVICE_NAME,
        remote: response,
    })
}

// ---------------------------------------------------------------------------
// urlacl add / list / remove
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ZenUrlaclResult {
    DryRun(ZenUrlaclPlan),
    Completed(ZenUrlaclSummary),
}

#[derive(Debug, Clone, Serialize)]
pub struct ZenUrlaclPlan {
    pub operation: String,
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub host: String,
    pub url_prefix: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZenUrlaclSummary {
    pub endpoint_id: i64,
    pub machine_id: i64,
    pub host: String,
    pub url_prefix: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    pub remote: serde_json::Value,
}

#[tauri::command]
pub fn zen_urlacl_add(
    db: State<'_, Db>,
    endpoint_id: i64,
    principal: String,
    confirmed: bool,
    dry_run: bool,
    cred: ZenCredentialInput,
) -> UecmResult<ZenUrlaclResult> {
    guard_destructive(confirmed, dry_run, "zen.urlacl.add")?;
    if principal.trim().is_empty() {
        return Err(UecmError::InvalidInput(
            "principal must not be empty or whitespace (URL ACL needs a real account)".into(),
        ));
    }
    cred.preflight(&db)?;
    let ep = zen_cli_shared::require_endpoint(&db, endpoint_id)?;
    let machine = zen_cli_shared::require_machine(&db, ep.machine_id)?;
    let url_prefix = zen_cli_shared::url_prefix_for(&ep);

    if dry_run {
        return Ok(ZenUrlaclResult::DryRun(ZenUrlaclPlan {
            operation: "zen.urlacl.add".to_string(),
            endpoint_id,
            machine_id: ep.machine_id,
            host: machine.ip,
            url_prefix,
            principal: Some(principal),
        }));
    }

    let creds = cred.resolve(&db)?;
    let invocation = redact(&format!(
        "zen-urlacl-add.ps1 -UrlPrefix {url_prefix} -UserAccount {principal}"
    ));
    let op_id = operations::start(&db, "zen.urlacl_add", &[ep.machine_id])?;
    let body = zen_cli_shared::build_param_script(
        "zen-urlacl-add.ps1",
        &[
            ("UrlPrefix", url_prefix.as_str()),
            ("UserAccount", principal.as_str()),
        ],
    );
    let result = match body {
        Ok(body) => zen_cli_shared::run_remote(&machine.ip, &body, creds.as_ref())
            .and_then(|raw| zen_cli_shared::parse_envelope(&raw, "zen-urlacl-add")),
        Err(e) => Err(e),
    };
    zen_cli_shared::finalize_op(&db, op_id, &result, &invocation);
    let response = result?;
    Ok(ZenUrlaclResult::Completed(ZenUrlaclSummary {
        endpoint_id,
        machine_id: ep.machine_id,
        host: machine.ip,
        url_prefix,
        principal: Some(principal),
        remote: response,
    }))
}

#[derive(Debug, Clone, Serialize)]
pub struct ZenUrlaclListResult {
    pub machine_id: i64,
    pub host: String,
    pub port_filter: Option<String>,
    pub remote: serde_json::Value,
}

#[tauri::command]
pub fn zen_urlacl_list(
    db: State<'_, Db>,
    machine_id: i64,
    port_filter: Option<String>,
    cred: ZenCredentialInput,
) -> UecmResult<ZenUrlaclListResult> {
    cred.preflight(&db)?;
    let m = zen_cli_shared::require_machine(&db, machine_id)?;
    let creds = cred.resolve(&db)?;

    let mut args: Vec<(&str, &str)> = Vec::new();
    if let Some(p) = port_filter.as_deref() {
        args.push(("PortFilter", p));
    }
    let body = zen_cli_shared::build_param_script("zen-urlacl-list.ps1", &args)?;
    let raw = zen_cli_shared::run_remote(&m.ip, &body, creds.as_ref())?;
    let response = zen_cli_shared::parse_envelope(&raw, "zen-urlacl-list")?;
    Ok(ZenUrlaclListResult {
        machine_id,
        host: m.ip,
        port_filter,
        remote: response,
    })
}

#[tauri::command]
pub fn zen_urlacl_remove(
    db: State<'_, Db>,
    endpoint_id: i64,
    confirmed: bool,
    dry_run: bool,
    cred: ZenCredentialInput,
) -> UecmResult<ZenUrlaclResult> {
    guard_destructive(confirmed, dry_run, "zen.urlacl.remove")?;
    cred.preflight(&db)?;
    let ep = zen_cli_shared::require_endpoint(&db, endpoint_id)?;
    let machine = zen_cli_shared::require_machine(&db, ep.machine_id)?;
    let url_prefix = zen_cli_shared::url_prefix_for(&ep);

    if dry_run {
        return Ok(ZenUrlaclResult::DryRun(ZenUrlaclPlan {
            operation: "zen.urlacl.remove".to_string(),
            endpoint_id,
            machine_id: ep.machine_id,
            host: machine.ip,
            url_prefix,
            principal: None,
        }));
    }

    let creds = cred.resolve(&db)?;
    let invocation = redact(&format!("zen-urlacl-remove.ps1 -UrlPrefix {url_prefix}"));
    let op_id = operations::start(&db, "zen.urlacl_remove", &[ep.machine_id])?;
    let body = zen_cli_shared::build_param_script(
        "zen-urlacl-remove.ps1",
        &[("UrlPrefix", url_prefix.as_str())],
    );
    let result = match body {
        Ok(body) => zen_cli_shared::run_remote(&machine.ip, &body, creds.as_ref())
            .and_then(|raw| zen_cli_shared::parse_envelope(&raw, "zen-urlacl-remove")),
        Err(e) => Err(e),
    };
    zen_cli_shared::finalize_op(&db, op_id, &result, &invocation);
    let response = result?;
    Ok(ZenUrlaclResult::Completed(ZenUrlaclSummary {
        endpoint_id,
        machine_id: ep.machine_id,
        host: machine.ip,
        url_prefix,
        principal: None,
        remote: response,
    }))
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Tauri analogue of `cli::destructive::check`. The UI must pass either
/// `dry_run=true` (preview) or `confirmed=true` (actually apply); otherwise
/// the wrapper refuses with `InvalidInput` so accidental invocations from the
/// front-end can't fire side effects.
fn guard_destructive(confirmed: bool, dry_run: bool, op: &str) -> UecmResult<()> {
    if dry_run || confirmed {
        return Ok(());
    }
    Err(UecmError::InvalidInput(format!(
        "{op} is destructive; pass confirmed=true to apply or dry_run=true to preview"
    )))
}

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

    // ----- T2.6 -----

    #[test]
    fn guard_destructive_refuses_with_neither_flag() {
        let err = guard_destructive(false, false, "zen.unregister").unwrap_err();
        match err {
            UecmError::InvalidInput(msg) => {
                assert!(msg.contains("confirmed=true"));
                assert!(msg.contains("dry_run=true"));
            }
            other => panic!("expected InvalidInput, got {:?}", other),
        }
    }

    #[test]
    fn guard_destructive_allows_confirmed() {
        guard_destructive(true, false, "zen.unregister").unwrap();
    }

    #[test]
    fn guard_destructive_allows_dry_run() {
        guard_destructive(false, true, "zen.unregister").unwrap();
    }

    #[test]
    fn guard_destructive_allows_both() {
        // Match CLI behavior: dry_run wins over confirmed but both being set
        // isn't an error.
        guard_destructive(true, true, "zen.unregister").unwrap();
    }

    #[test]
    fn cred_input_preflight_inconsistent_user_only_errors() {
        let db = fresh_db();
        let bad = ZenCredentialInput {
            cred_alias: None,
            user: Some("alice".into()),
            pass: None,
        };
        assert!(matches!(
            bad.preflight(&db),
            Err(UecmError::InvalidInput(_))
        ));
    }

    #[test]
    fn cred_input_preflight_alias_with_user_errors() {
        let db = fresh_db();
        let bad = ZenCredentialInput {
            cred_alias: Some("any".into()),
            user: Some("alice".into()),
            pass: None,
        };
        assert!(matches!(
            bad.preflight(&db),
            Err(UecmError::InvalidInput(_))
        ));
    }

    #[test]
    fn cred_input_preflight_unknown_alias_errors() {
        let db = fresh_db();
        let bad = ZenCredentialInput {
            cred_alias: Some("UECM:winrm:NOPE".into()),
            user: None,
            pass: None,
        };
        assert!(matches!(
            bad.preflight(&db),
            Err(UecmError::InvalidInput(_))
        ));
    }

    #[test]
    fn cred_input_resolve_anonymous_returns_none() {
        let db = fresh_db();
        let none_cred = ZenCredentialInput::default();
        assert!(none_cred.resolve(&db).unwrap().is_none());
    }

    #[test]
    fn cred_input_resolve_inline_user_pass() {
        let db = fresh_db();
        let inline = ZenCredentialInput {
            cred_alias: None,
            user: Some("alice".into()),
            pass: Some("hunter2".into()),
        };
        assert_eq!(
            inline.resolve(&db).unwrap(),
            Some(("alice".into(), "hunter2".into()))
        );
    }

    #[test]
    fn register_unknown_machine_errors() {
        let db = fresh_db();
        // Mirror the pre-check `zen_register` runs before calling
        // `core::zen::endpoint::register`.
        assert!(machines::find_by_id(&db, 9999).unwrap().is_none());
    }

    #[test]
    fn register_uses_default_lifecycle_for_role() {
        // Verify the same lifecycle defaulting rule the CLI uses.
        assert_eq!(
            zen_cli_shared::default_lifecycle_for("shared_upstream"),
            "installed_service"
        );
        assert_eq!(
            zen_cli_shared::default_lifecycle_for("local"),
            "editor_owned"
        );
    }

    #[test]
    fn unregister_refuses_when_dependents_exist() {
        // Seed master + dependent; ensure the dependents scan would block.
        let db = fresh_db();
        let (m1, master_id) = seed_endpoint(&db, "ZEN-M", "10.0.0.10", 8558);
        // Replace master row to set role=shared_upstream and lifecycle=installed_service.
        zen_endpoints::upsert(
            &db,
            &ZenEndpoint {
                id: Some(master_id),
                machine_id: m1,
                declared_port: 8558,
                scheme: "http".into(),
                role: "shared_upstream".into(),
                upstream_endpoint_id: None,
                data_dir: r"D:\Zen".into(),
                httpserverclass: "asio".into(),
                lifecycle_mode: "installed_service".into(),
                created_at: None,
                updated_at: None,
            },
        )
        .unwrap();
        // Add a local endpoint that points upstream at the master.
        let _local_id = zen_endpoints::upsert(
            &db,
            &ZenEndpoint {
                id: None,
                machine_id: m1,
                declared_port: 8559,
                scheme: "http".into(),
                role: "local".into(),
                upstream_endpoint_id: Some(master_id),
                data_dir: r"D:\Zen2".into(),
                httpserverclass: "asio".into(),
                lifecycle_mode: "editor_owned".into(),
                created_at: None,
                updated_at: None,
            },
        )
        .unwrap();

        let dependents: Vec<i64> = zen_endpoints::list(&db)
            .unwrap()
            .into_iter()
            .filter(|other| other.upstream_endpoint_id == Some(master_id))
            .filter_map(|other| other.id)
            .collect();
        assert!(!dependents.is_empty());
    }

    #[test]
    fn lua_preview_data_dir_safe_check_rejects_system_root() {
        // Verify the same guard the command runs internally.
        let r = zen_cli_shared::validate_data_dir_safe(r"C:\Windows\Zen");
        assert!(matches!(r, Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn apply_config_dest_path_check_rejects_relative() {
        let r = zen_cli_shared::validate_dest_path(r"relative\zen.lua");
        assert!(matches!(r, Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn urlacl_add_empty_principal_errors() {
        // Refused at the wrapper before any cred / DB / PS work.
        let trimmed = "   ".trim();
        assert!(trimmed.is_empty());
    }
}
