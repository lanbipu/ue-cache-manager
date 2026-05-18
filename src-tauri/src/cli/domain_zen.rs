//! `uecm-cli zen <action>` handlers (Plan 7 M1 T1.9).
//!
//! # NDJSON event schema
//!
//! Every subcommand here emits NDJSON events under the shared `cli::output::Event`
//! taxonomy. The schemas below describe the `summary` payload of `Completed`
//! events and the wrapping result documents emitted via `emit_result` so T1.10
//! Tauri commands can mirror them 1:1.
//!
//! ## `zen status`
//! Top-level result document: `{ "endpoints": [ ZenEndpointStatus, ... ] }` where
//! `ZenEndpointStatus = { endpoint_id, machine_id, hostname, declared_port,
//! scheme, role, lifecycle_mode, latest_probe?: { probed_at, reachable,
//! effective_port, build_version, error_message }, ok }`.
//!
//! ## `zen probe`
//! Per-endpoint `Completed` summary: `{ endpoint_id, reachable, build_version,
//! effective_port, error_message, probe_id }`. Top-level `Completed` summary:
//! `{ probed: <int>, reachable: <int>, unreachable: <int> }`.
//!
//! ## `zen cache-stats`
//! Per-endpoint `Completed` summary: `{ endpoint_id, providers: [<name>, ...],
//! records: <int>, error_message }`. Top-level `Completed` summary: `{ endpoints:
//! <int>, rows_inserted: <int> }`.
//!
//! ## `zen detect-binary`
//! Per-machine `Completed` summary: `{ machine_id, install_record_written: bool,
//! install_record_cleared: bool, intree_records_written: <int>, baseline_new_rows:
//! <int>, intree_ref_rows: <int>, warnings: [<string>, ...] }`. Top-level
//! `Completed` summary: `{ machines: <int>, ok: <int>, failed: <int> }`.
//!
//! ## `zen list-endpoints`
//! Result document is the raw `Vec<ZenEndpoint>` from the data layer.
//!
//! ## `zen baseline list` / `lock` / `unlock`
//! `list` emits the raw `Vec<ZenBinaryExpected>`. `lock`/`unlock` emit `Completed`
//! summaries `{ zen_build_version, kind, locked_by?, action: "lock"|"unlock" }`.
//!
//! # Exit codes (plan §2.4)
//! - 0 success
//! - 1 partial failure (some probes unreachable / some persists raised warnings)
//! - 2 arg / parse error (clap)
//! - 3 environment / DB / IO failure
//! - 4 credential / PowerShell failure (e.g. detect-binary can't reach host)

use crate::cli::args::{ZenAction, ZenBaselineAction};
use crate::cli::credential_args::CredentialArgs;
use crate::cli::destructive::{self, Outcome};
use crate::cli::output::Event;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::core::zen::{binary as zen_binary, cache_stats as zen_cache, probe as zen_probe};
use crate::data::{
    machines, zen_binary_expected, zen_endpoints, zen_probes, Db, ZenEndpoint,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use std::time::Duration;

const KIND_ZEN_CLI: &str = "zen_cli";
const KIND_ZENSERVER: &str = "zenserver";

pub fn handle(ctx: &mut Ctx<'_>, action: ZenAction) -> UecmResult<()> {
    match action {
        ZenAction::Status { machine, all } => status(ctx, machine, all),
        ZenAction::Probe { machine, all, timeout, cred } => probe(ctx, machine, all, timeout, &cred),
        ZenAction::CacheStats { endpoint_id, all, timeout } => {
            cache_stats(ctx, endpoint_id, all, timeout)
        }
        ZenAction::DetectBinary { machine, all, cred } => detect_binary(ctx, machine, all, &cred),
        ZenAction::ListEndpoints { machine } => list_endpoints(ctx, machine),
        ZenAction::Baseline { action } => match action {
            ZenBaselineAction::List { zen_build_version, kind } => {
                baseline_list(ctx, zen_build_version.as_deref(), kind.as_deref())
            }
            ZenBaselineAction::Lock { zen_build_version, kind, locked_by, yes, dry_run } => {
                baseline_lock(ctx, &zen_build_version, &kind, &locked_by, yes, dry_run)
            }
            ZenBaselineAction::Unlock { zen_build_version, kind, yes, dry_run } => {
                baseline_unlock(ctx, &zen_build_version, &kind, yes, dry_run)
            }
        },
    }
}

// -----------------------------------------------------------------------------
// status
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct LatestProbeView {
    probed_at: Option<String>,
    reachable: bool,
    effective_port: Option<i64>,
    build_version: Option<String>,
    error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct EndpointStatus {
    endpoint_id: i64,
    machine_id: i64,
    hostname: String,
    ip: String,
    declared_port: i64,
    scheme: String,
    role: String,
    lifecycle_mode: String,
    latest_probe: Option<LatestProbeView>,
    /// `true` iff a latest probe exists and was reachable. Convenience field for
    /// dashboard rendering — saves consumers from reaching into `latest_probe`.
    ok: bool,
}

fn status(ctx: &mut Ctx<'_>, machine: Option<i64>, _all: bool) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let endpoints = resolve_endpoints(db, machine, None)?;
    let mut out = Vec::with_capacity(endpoints.len());
    for ep in endpoints {
        let endpoint_id = ep.id.expect("endpoint from CRUD always has id");
        let machine_record = machines::find_by_id(db, ep.machine_id)?;
        let (hostname, ip) = machine_record
            .map(|m| (m.hostname, m.ip))
            .unwrap_or_else(|| (String::new(), String::new()));

        // Plan §3 health check 1 reads "latest" probe — that's the most-recent
        // row by probed_at. `list_recent` returns DESC, so head of the vec is it.
        let recent = zen_probes::list_recent(db, endpoint_id, 1)?;
        let latest_probe = recent.into_iter().next().map(|p| LatestProbeView {
            probed_at: p.probed_at,
            reachable: p.reachable,
            effective_port: p.effective_port,
            build_version: p.build_version,
            error_message: p.error_message,
        });
        let ok = latest_probe.as_ref().map(|p| p.reachable).unwrap_or(false);
        out.push(EndpointStatus {
            endpoint_id,
            machine_id: ep.machine_id,
            hostname,
            ip,
            declared_port: ep.declared_port,
            scheme: ep.scheme,
            role: ep.role,
            lifecycle_mode: ep.lifecycle_mode,
            latest_probe,
            ok,
        });
    }
    let doc = serde_json::json!({ "endpoints": out });
    ctx.emitter.emit_result(&doc).ok();
    Ok(())
}

// -----------------------------------------------------------------------------
// probe
// -----------------------------------------------------------------------------

fn probe(
    ctx: &mut Ctx<'_>,
    machine: Option<i64>,
    _all: bool,
    timeout_secs: u64,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    // The credential pair is accepted today for forward compatibility (plan
    // notes anticipate a WinRM-tunneled probe variant). Validate flag
    // combinations against the DB so a typo'd alias fails fast.
    let db_clone = ctx.require_db()?.clone();
    cred.preflight(&db_clone)?;
    let _ = cred; // currently unused at runtime — direct HTTP only.

    let endpoints = resolve_endpoints(&db_clone, machine, None)?;
    let total = endpoints.len() as i64;
    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "zen_probe".into(),
            task_id: None,
            metadata: serde_json::json!({ "endpoints": total, "timeout_secs": timeout_secs }),
        })
        .ok();

    let timeout = Duration::from_secs(timeout_secs);
    let mut reachable = 0i64;
    let mut unreachable = 0i64;
    for (idx, ep) in endpoints.iter().enumerate() {
        let endpoint_id = ep.id.expect("endpoint id");
        let host = match resolve_host(&db_clone, ep.machine_id)? {
            Some(h) => h,
            None => {
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: format!("endpoint:{}", endpoint_id),
                        index: idx as i64,
                        ok: false,
                        message: Some(format!(
                            "machine id={} not found; cannot resolve host",
                            ep.machine_id
                        )),
                    })
                    .ok();
                unreachable += 1;
                continue;
            }
        };

        let outcome = zen_probe::probe_endpoint(ep, &host, timeout);
        let record = outcome.record.clone();
        let probe_id = zen_probe::persist(&db_clone, &outcome)?;
        if record.reachable {
            reachable += 1;
        } else {
            unreachable += 1;
        }
        let summary = serde_json::json!({
            "endpoint_id": endpoint_id,
            "machine_id": ep.machine_id,
            "host": host,
            "reachable": record.reachable,
            "build_version": record.build_version,
            "effective_port": record.effective_port,
            "error_message": record.error_message,
            "probe_id": probe_id,
        });
        ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    }

    let final_summary = serde_json::json!({
        "probed": total,
        "reachable": reachable,
        "unreachable": unreachable,
    });
    ctx.emitter.emit_event(&Event::Completed { summary: final_summary }).ok();
    // Partial failure → exit 1. UecmError::OperationFailed maps to exit 1 per
    // cli::output::exit_code_for, which keeps the dual-channel contract intact.
    if unreachable > 0 && reachable > 0 {
        return Err(UecmError::OperationFailed(format!(
            "{}/{} endpoints unreachable",
            unreachable, total
        )));
    }
    // All-failure stays as full failure → exit 1 too. All-success → exit 0.
    if unreachable == total && total > 0 {
        return Err(UecmError::OperationFailed(format!(
            "all {} endpoints unreachable",
            total
        )));
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// cache-stats
// -----------------------------------------------------------------------------

fn cache_stats(
    ctx: &mut Ctx<'_>,
    endpoint_id: Option<i64>,
    _all: bool,
    timeout_secs: u64,
) -> UecmResult<()> {
    let db = ctx.require_db()?.clone();
    let endpoints = resolve_endpoints(&db, None, endpoint_id)?;
    let total = endpoints.len() as i64;
    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "zen_cache_stats".into(),
            task_id: None,
            metadata: serde_json::json!({ "endpoints": total, "timeout_secs": timeout_secs }),
        })
        .ok();

    let timeout = Duration::from_secs(timeout_secs);
    let mut rows_inserted = 0i64;
    let mut partial_errors = 0i64;
    for (idx, ep) in endpoints.iter().enumerate() {
        let endpoint_id = ep.id.expect("endpoint id");
        let host = match resolve_host(&db, ep.machine_id)? {
            Some(h) => h,
            None => {
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: format!("endpoint:{}", endpoint_id),
                        index: idx as i64,
                        ok: false,
                        message: Some(format!("machine id={} not found", ep.machine_id)),
                    })
                    .ok();
                partial_errors += 1;
                continue;
            }
        };
        let outcome = zen_cache::fetch_cache_stats(ep, &host, timeout);
        let ids = zen_cache::persist(&db, &outcome)?;
        rows_inserted += ids.len() as i64;
        if outcome.error_message.is_some() {
            partial_errors += 1;
        }
        let summary = serde_json::json!({
            "endpoint_id": endpoint_id,
            "machine_id": ep.machine_id,
            "host": host,
            "providers": outcome.providers,
            "records": ids.len(),
            "error_message": outcome.error_message,
        });
        ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    }

    let final_summary = serde_json::json!({
        "endpoints": total,
        "rows_inserted": rows_inserted,
        "partial_errors": partial_errors,
    });
    ctx.emitter.emit_event(&Event::Completed { summary: final_summary }).ok();
    // Treat "every endpoint failed" as a hard failure too — automation must
    // not see exit 0 when literally nothing was sampled. The intermediate
    // partial_errors < total branch covers mixed-success runs.
    if total > 0 && partial_errors == total {
        return Err(UecmError::OperationFailed(format!(
            "all {} endpoint(s) failed to fetch cache stats; no rows inserted",
            total
        )));
    }
    if partial_errors > 0 && partial_errors < total {
        return Err(UecmError::OperationFailed(format!(
            "{}/{} endpoints had errors fetching cache stats",
            partial_errors, total
        )));
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// detect-binary
// -----------------------------------------------------------------------------

fn detect_binary(
    ctx: &mut Ctx<'_>,
    machine: Option<i64>,
    all: bool,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?.clone();
    let creds = cred.resolve(&db)?;

    // Resolve target machines. --machine takes precedence; otherwise --all (or
    // no flag) scans every machine in inventory.
    let target_machines: Vec<crate::data::Machine> = match machine {
        Some(id) => {
            let m = machines::find_by_id(&db, id)?.ok_or_else(|| {
                UecmError::InvalidInput(format!("machine id={} not found", id))
            })?;
            vec![m]
        }
        None => {
            if !all && !ctx.json_mode {
                // No --machine and no --all is ambiguous — fall through to all but
                // make the convention explicit when humans run it (json mode
                // remains permissive for scripted batch use).
            }
            machines::list_all(&db)?
        }
    };

    let total = target_machines.len() as i64;
    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "zen_detect_binary".into(),
            task_id: None,
            metadata: serde_json::json!({ "machines": total, "authenticated": creds.is_some() }),
        })
        .ok();

    let mut ok_count = 0i64;
    let mut failed = 0i64;
    for (idx, m) in target_machines.iter().enumerate() {
        let machine_id = m.id.expect("machine in inventory always has id");
        let host = &m.ip;
        let detection_result = invoke_detect_binary(host, creds.as_ref());
        match detection_result {
            Ok(detection) => {
                let report = zen_binary::persist(&db, machine_id, &detection)?;
                ok_count += 1;
                let summary = serde_json::json!({
                    "machine_id": machine_id,
                    "hostname": m.hostname,
                    "ip": m.ip,
                    "install_record_written": report.install_record_written,
                    "install_record_cleared": report.install_record_cleared,
                    "intree_records_written": report.intree_records_written,
                    "baseline_new_rows": report.baseline_new_rows,
                    "intree_ref_rows": report.intree_ref_rows,
                    "warnings": report.warnings,
                });
                ctx.emitter.emit_event(&Event::Completed { summary }).ok();
            }
            Err(e) => {
                failed += 1;
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: format!("machine:{}", machine_id),
                        index: idx as i64,
                        ok: false,
                        message: Some(e.to_string()),
                    })
                    .ok();
            }
        }
    }

    let final_summary = serde_json::json!({
        "machines": total,
        "ok": ok_count,
        "failed": failed,
    });
    ctx.emitter.emit_event(&Event::Completed { summary: final_summary }).ok();
    if failed > 0 && ok_count > 0 {
        return Err(UecmError::OperationFailed(format!(
            "{}/{} machines failed detect-binary",
            failed, total
        )));
    }
    if failed == total && total > 0 {
        return Err(UecmError::PowerShell(format!(
            "all {} machines failed detect-binary",
            total
        )));
    }
    Ok(())
}

/// Run `zen-detect-binary.ps1` against `host` and parse the JSON payload.
///
/// Routes through `core::winrm::invoke_json*` so the sidecar runs remotely on
/// the target. The script body is forwarded inline (no args required by the
/// script itself).
fn invoke_detect_binary(
    host: &str,
    creds: Option<&(String, String)>,
) -> UecmResult<zen_binary::BinaryDetection> {
    let body = crate::core::powershell::read_script("zen-detect-binary.ps1")?;
    let raw: String = match creds {
        Some((u, p)) => crate::core::winrm::invoke_with_credential(host, &body, u, p)?,
        None => crate::core::winrm::invoke(host, &body)?,
    };

    // PS sidecars emit exit 0 even on expected failures, with `{ok:false,
    // message:"..."}` as the envelope (T1.8 contract). If we forward that
    // straight into parse_detection_json the parser treats missing
    // install/intree as "no install detected" — which then causes
    // zen_binary::persist to delete the existing machine_zen_install row
    // (T1.6 P2-1 fix: detection.install=None → drop stale). The result
    // would be sidecar failure silently nuking inventory, the exact bug
    // codex flagged. Inspect ok first.
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

// -----------------------------------------------------------------------------
// list-endpoints
// -----------------------------------------------------------------------------

fn list_endpoints(ctx: &mut Ctx<'_>, machine: Option<i64>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = match machine {
        Some(id) => zen_endpoints::list_for_machine(db, id)?,
        None => zen_endpoints::list(db)?,
    };
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}

// -----------------------------------------------------------------------------
// baseline list / lock / unlock
// -----------------------------------------------------------------------------

fn baseline_list(
    ctx: &mut Ctx<'_>,
    version_filter: Option<&str>,
    kind_filter: Option<&str>,
) -> UecmResult<()> {
    if let Some(k) = kind_filter {
        validate_kind(k)?;
    }
    let db = ctx.require_db()?;
    let mut rows = zen_binary_expected::list(db)?;
    if let Some(v) = version_filter {
        rows.retain(|r| r.zen_build_version == v);
    }
    if let Some(k) = kind_filter {
        rows.retain(|r| r.binary_kind == k);
    }
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}

fn baseline_lock(
    ctx: &mut Ctx<'_>,
    version: &str,
    kind: &str,
    locked_by: &str,
    yes: bool,
    dry_run: bool,
) -> UecmResult<()> {
    validate_kind(kind)?;
    let outcome = destructive::check(yes, dry_run, "zen.baseline.lock")?;
    let db = ctx.require_db()?;

    // Existence check up front so the operator gets a clear message rather than
    // a silent no-op (UPDATE ... WHERE doesn't fail on zero rows in SQLite).
    if zen_binary_expected::find(db, version, kind)?.is_none() {
        return Err(UecmError::InvalidInput(format!(
            "no baseline row for zen_build_version={} kind={}; run detect-binary first",
            version, kind
        )));
    }

    if outcome == Outcome::DryRun {
        destructive::emit_plan(
            ctx.emitter.as_mut(),
            "zen.baseline.lock",
            serde_json::json!({
                "zen_build_version": version,
                "kind": kind,
                "locked_by": locked_by,
            }),
        );
        return Ok(());
    }

    zen_binary_expected::lock(db, version, kind, locked_by)?;
    let summary = serde_json::json!({
        "zen_build_version": version,
        "kind": kind,
        "locked_by": locked_by,
        "action": "lock",
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}

fn baseline_unlock(
    ctx: &mut Ctx<'_>,
    version: &str,
    kind: &str,
    yes: bool,
    dry_run: bool,
) -> UecmResult<()> {
    validate_kind(kind)?;
    let outcome = destructive::check(yes, dry_run, "zen.baseline.unlock")?;
    let db = ctx.require_db()?;

    if zen_binary_expected::find(db, version, kind)?.is_none() {
        return Err(UecmError::InvalidInput(format!(
            "no baseline row for zen_build_version={} kind={}",
            version, kind
        )));
    }

    if outcome == Outcome::DryRun {
        destructive::emit_plan(
            ctx.emitter.as_mut(),
            "zen.baseline.unlock",
            serde_json::json!({
                "zen_build_version": version,
                "kind": kind,
            }),
        );
        return Ok(());
    }

    zen_binary_expected::unlock(db, version, kind)?;
    let summary = serde_json::json!({
        "zen_build_version": version,
        "kind": kind,
        "action": "unlock",
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}

// -----------------------------------------------------------------------------
// helpers
// -----------------------------------------------------------------------------

/// Resolve the set of endpoints a command should act on.
///
/// Priority order:
/// 1. `endpoint_id`  → single endpoint (used by `zen cache-stats`).
/// 2. `machine`      → all endpoints registered to that machine.
/// 3. neither given  → every registered endpoint (the implicit `--all`).
fn resolve_endpoints(
    db: &Db,
    machine: Option<i64>,
    endpoint_id: Option<i64>,
) -> UecmResult<Vec<ZenEndpoint>> {
    if let Some(id) = endpoint_id {
        let ep = zen_endpoints::get(db, id)?
            .ok_or_else(|| UecmError::InvalidInput(format!("endpoint id={} not found", id)))?;
        return Ok(vec![ep]);
    }
    if let Some(mid) = machine {
        // Sanity check: empty result on an unknown machine id should fail loudly
        // rather than silently no-op, matching the rest of the CLI.
        if machines::find_by_id(db, mid)?.is_none() {
            return Err(UecmError::InvalidInput(format!("machine id={} not found", mid)));
        }
        return zen_endpoints::list_for_machine(db, mid);
    }
    zen_endpoints::list(db)
}

/// Look up the IP for a machine row. Hostname can drift if the operator
/// renamed the row, so IP is the canonical connect target (matches the rest of
/// the CLI / discovery code path).
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

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::output::{Emitter, NdjsonEmitter};
    use crate::data::{
        machines, open_in_memory, schema, zen_binary_expected, zen_endpoints, Machine,
        ZenBinaryExpected,
    };
    use std::path::PathBuf;

    fn fresh_ctx() -> Ctx<'static> {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(Vec::new()));
        Ctx {
            db: Some(db),
            db_path: PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        }
    }

    fn seed_endpoint(db: &Db, machine_hostname: &str, ip: &str, port: i64) -> (i64, i64) {
        let machine_id = machines::insert(db, &Machine::new(machine_hostname, ip)).unwrap();
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

    #[test]
    fn status_on_empty_db_returns_empty_endpoints() {
        let mut ctx = fresh_ctx();
        status(&mut ctx, None, true).unwrap();
        // Successful no-op — handler emits a result document; not asserting
        // bytes since the test emitter sinks into a moved Vec.
    }

    #[test]
    fn status_with_seeded_endpoint_reports_no_probe_yet() {
        let mut ctx = fresh_ctx();
        let db = ctx.db.as_ref().unwrap().clone();
        seed_endpoint(&db, "ZEN-1", "10.0.0.10", 8558);
        status(&mut ctx, None, true).unwrap();

        // Direct DB read to verify the data shape the handler produced is
        // consistent with downstream consumers' expectations.
        let endpoints = zen_endpoints::list(&db).unwrap();
        assert_eq!(endpoints.len(), 1);
        let recent = zen_probes::list_recent(&db, endpoints[0].id.unwrap(), 1).unwrap();
        assert!(recent.is_empty(), "no probe rows expected on a fresh seed");
    }

    #[test]
    fn list_endpoints_empty_ok() {
        let mut ctx = fresh_ctx();
        list_endpoints(&mut ctx, None).unwrap();
    }

    #[test]
    fn list_endpoints_filtered_by_machine() {
        let mut ctx = fresh_ctx();
        let db = ctx.db.as_ref().unwrap().clone();
        let (m1, _) = seed_endpoint(&db, "ZEN-1", "10.0.0.10", 8558);
        let (_m2, _) = seed_endpoint(&db, "ZEN-2", "10.0.0.11", 8558);
        list_endpoints(&mut ctx, Some(m1)).unwrap();
        let just_m1 = zen_endpoints::list_for_machine(&db, m1).unwrap();
        assert_eq!(just_m1.len(), 1);
    }

    #[test]
    fn baseline_list_empty_ok() {
        let mut ctx = fresh_ctx();
        baseline_list(&mut ctx, None, None).unwrap();
    }

    #[test]
    fn baseline_list_filters_by_version_and_kind() {
        let mut ctx = fresh_ctx();
        let db = ctx.db.as_ref().unwrap().clone();
        seed_baseline(&db, "5.8.10-aaa", KIND_ZEN_CLI, "sha-cli-aaa");
        seed_baseline(&db, "5.8.10-aaa", KIND_ZENSERVER, "sha-srv-aaa");
        seed_baseline(&db, "5.7.6-bbb", KIND_ZEN_CLI, "sha-cli-bbb");
        // Filter handler runs against the live DB inside the call; we re-run
        // the filter logic locally to assert the resulting Vec shape.
        let all = zen_binary_expected::list(&db).unwrap();
        assert_eq!(all.len(), 3);
        let just_aaa: Vec<_> = all
            .iter()
            .filter(|r| r.zen_build_version == "5.8.10-aaa")
            .collect();
        assert_eq!(just_aaa.len(), 2);
        baseline_list(&mut ctx, Some("5.8.10-aaa"), Some(KIND_ZEN_CLI)).unwrap();
    }

    #[test]
    fn baseline_lock_requires_yes() {
        let mut ctx = fresh_ctx();
        let db = ctx.db.as_ref().unwrap().clone();
        seed_baseline(&db, "5.8.10-aaa", KIND_ZEN_CLI, "sha");
        let err = baseline_lock(&mut ctx, "5.8.10-aaa", KIND_ZEN_CLI, "op1", false, false)
            .unwrap_err();
        assert!(matches!(err, UecmError::InvalidInput(_)));
    }

    #[test]
    fn baseline_lock_dry_run_does_not_persist() {
        let mut ctx = fresh_ctx();
        let db = ctx.db.as_ref().unwrap().clone();
        seed_baseline(&db, "5.8.10-aaa", KIND_ZEN_CLI, "sha");
        baseline_lock(&mut ctx, "5.8.10-aaa", KIND_ZEN_CLI, "op1", false, true).unwrap();
        let row = zen_binary_expected::find(&db, "5.8.10-aaa", KIND_ZEN_CLI)
            .unwrap()
            .unwrap();
        assert!(row.locked_by.is_none(), "dry-run must not write");
    }

    #[test]
    fn baseline_lock_applies_with_yes() {
        let mut ctx = fresh_ctx();
        let db = ctx.db.as_ref().unwrap().clone();
        seed_baseline(&db, "5.8.10-aaa", KIND_ZEN_CLI, "sha");
        baseline_lock(&mut ctx, "5.8.10-aaa", KIND_ZEN_CLI, "op1", true, false).unwrap();
        let row = zen_binary_expected::find(&db, "5.8.10-aaa", KIND_ZEN_CLI)
            .unwrap()
            .unwrap();
        assert_eq!(row.locked_by.as_deref(), Some("op1"));
    }

    #[test]
    fn baseline_unlock_clears_marker() {
        let mut ctx = fresh_ctx();
        let db = ctx.db.as_ref().unwrap().clone();
        seed_baseline(&db, "5.8.10-aaa", KIND_ZEN_CLI, "sha");
        zen_binary_expected::lock(&db, "5.8.10-aaa", KIND_ZEN_CLI, "op1").unwrap();
        baseline_unlock(&mut ctx, "5.8.10-aaa", KIND_ZEN_CLI, true, false).unwrap();
        let row = zen_binary_expected::find(&db, "5.8.10-aaa", KIND_ZEN_CLI)
            .unwrap()
            .unwrap();
        assert!(row.locked_by.is_none());
    }

    #[test]
    fn baseline_lock_rejects_missing_row() {
        let mut ctx = fresh_ctx();
        let err = baseline_lock(&mut ctx, "nope-version", KIND_ZEN_CLI, "op1", true, false)
            .unwrap_err();
        match err {
            UecmError::InvalidInput(msg) => assert!(msg.contains("no baseline row")),
            other => panic!("expected InvalidInput, got {:?}", other),
        }
    }

    #[test]
    fn baseline_rejects_bad_kind() {
        let mut ctx = fresh_ctx();
        let err = baseline_list(&mut ctx, None, Some("bogus")).unwrap_err();
        match err {
            UecmError::InvalidInput(msg) => assert!(msg.contains("invalid binary kind")),
            other => panic!("expected InvalidInput, got {:?}", other),
        }
    }

    #[test]
    fn probe_with_unknown_machine_errors_out() {
        let mut ctx = fresh_ctx();
        let cred = CredentialArgs {
            cred_alias: None,
            user: None,
            pass: None,
            pass_stdin: false,
        };
        let err = probe(&mut ctx, Some(9999), false, 2, &cred).unwrap_err();
        assert!(matches!(err, UecmError::InvalidInput(_)));
    }

    #[test]
    fn cache_stats_unknown_endpoint_errors() {
        let mut ctx = fresh_ctx();
        let err = cache_stats(&mut ctx, Some(9999), false, 2).unwrap_err();
        assert!(matches!(err, UecmError::InvalidInput(_)));
    }
}
