//! Orchestrate per-machine probes + cluster-level aggregators (GPU, INI consistency).

use crate::core::cache_backend::ZEN_PROBE_FRESHNESS_WINDOW;
use crate::core::network;
use crate::data::machine_gpus::GpuInfo;
use crate::data::{
    machine_zen_install, zen_binary_expected, zen_endpoints, Db,
};
use crate::error::UecmResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckOutcome {
    pub status: String,
    pub message: String,
    pub sample: String,
    #[serde(default)]
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuConsistencyReport {
    pub outcomes: HashMap<i64, CheckOutcome>,
}

pub fn aggregate_gpu_consistency(gpus: &[GpuInfo]) -> GpuConsistencyReport {
    let mut by_machine: HashMap<i64, &GpuInfo> = HashMap::new();
    for g in gpus { by_machine.insert(g.machine_id, g); }

    let mut combo_counts: HashMap<(String, String), i64> = HashMap::new();
    for g in by_machine.values() {
        *combo_counts
            .entry((g.gpu_model.clone(), g.driver_version.clone()))
            .or_insert(0) += 1;
    }
    let model_counts: HashMap<String, i64> = {
        let mut m = HashMap::new();
        for g in by_machine.values() { *m.entry(g.gpu_model.clone()).or_insert(0) += 1; }
        m
    };

    let mut outcomes = HashMap::new();
    for (mid, g) in &by_machine {
        let same_combo = combo_counts.get(&(g.gpu_model.clone(), g.driver_version.clone())).copied().unwrap_or(0);
        let same_model = model_counts.get(&g.gpu_model).copied().unwrap_or(0);
        let total = by_machine.len() as i64;
        let status = if total == 1 || same_combo == total {
            "healthy"
        } else if same_model == total {
            "warning"
        } else {
            "critical"
        };
        outcomes.insert(*mid, CheckOutcome {
            status: status.into(),
            message: format!(
                "{} {} ({} of {} machines have same combo)",
                g.gpu_model, g.driver_version, same_combo, total
            ),
            sample: format!("{} / {}", g.gpu_model, g.driver_version),
            remediation: if status == "critical" {
                "Standardize GPU + driver across cluster, or split into compatible subgroups before PSO distribute"
                    .into()
            } else if status == "warning" {
                "Make every node run the same NVIDIA driver: audit with `nvidia-smi --query`, then push a matched installer cluster-wide"
                    .into()
            } else {
                String::new()
            },
        });
    }
    GpuConsistencyReport { outcomes }
}

/// L1 (port-layer) probe. Runs from the operator console — no credentials,
/// no WinRM, just three TCP connect attempts. Returns three `CheckOutcome`
/// rows keyed `tcp_5985` / `tcp_445` / `tcp_135` with remediation strings
/// that direct the operator toward the right bootstrap path.
pub async fn probe_tcp_ports(host: &str, timeout_ms: u64) -> HashMap<String, CheckOutcome> {
    let probed = network::probe_host_one(host, timeout_ms).await;
    let mut out = HashMap::new();

    out.insert(
        "tcp_5985".into(),
        port_outcome(
            "WinRM 5985",
            probed.winrm_open,
            "Run `uecm-cli winrm bootstrap <host>` (Path B) when 445+135 are open, or use the USB Path A bootstrap when all three ports are closed.",
        ),
    );
    out.insert(
        "tcp_445".into(),
        port_outcome(
            "SMB 445",
            probed.smb_open,
            "Open inbound TCP 445 (FPS-SMB-In-TCP firewall rule) and start LanmanServer. `winrm bootstrap` does both via -EnableSmbServer.",
        ),
    );
    out.insert(
        "tcp_135".into(),
        port_outcome(
            "RPC 135 (Endpoint Mapper)",
            probed.rpc_open,
            "Switch network profile to Private (Public default blocks DCOM-In). `winrm bootstrap` does this when -NetworkCategory Private is passed.",
        ),
    );
    out
}

/// Per-machine zen health rows (Plan 7 §M4 T4.2).
///
/// Returns four rows keyed by string, each carrying a [`CheckOutcome`]:
///
/// * `zen_reachable` — at least one of this machine's zen endpoints had a
///   probe with `reachable=1` inside the freshness window
///   ([`ZEN_PROBE_FRESHNESS_WINDOW`]). No probes ever → critical with a
///   hint pointing at `zen probe`.
/// * `zen_version_consistent` — `machine_zen_install.zenserver_build_version`
///   matches the cluster majority. Outlier → warning. Cluster with fewer
///   than 3 machines → healthy (can't claim majority).
/// * `zen_binary_intact` — install-path sha256 matches the baseline in
///   `zen_binary_expected (zen_build_version, binary_kind="zenserver")`.
///   Mismatch on the install path → warning (operator should re-sync).
///   InTree drift is NOT surfaced here — Plan §M4 T4.3 explicitly says
///   "InTree 漂移仅日志不告警" (log only); callers wanting to log can use
///   `data::zen_binary_intree::list` separately.
/// * `zen_cache_provider_ready` — the latest `zen_cache_stats` row for any
///   of this machine's endpoints has `provider_path` covering `z$` (the
///   DDC namespace zen serves to UE). No stats ever → warning.
///
/// All four rows are always returned (never an empty map) so the UI can
/// show a stable layout. Status values are `"healthy"` / `"warning"` /
/// `"critical"` / `"unknown"`.
pub fn zen_health_for_machine(
    db: &Db,
    machine_id: i64,
) -> UecmResult<HashMap<String, CheckOutcome>> {
    let mut out = HashMap::new();
    out.insert("zen_reachable".into(), check_zen_reachable(db, machine_id)?);
    out.insert(
        "zen_version_consistent".into(),
        check_zen_version_consistent(db, machine_id)?,
    );
    out.insert(
        "zen_binary_intact".into(),
        check_zen_binary_intact(db, machine_id)?,
    );
    out.insert(
        "zen_cache_provider_ready".into(),
        check_zen_cache_provider_ready(db, machine_id)?,
    );
    Ok(out)
}

fn check_zen_reachable(db: &Db, machine_id: i64) -> UecmResult<CheckOutcome> {
    let endpoints = zen_endpoints::list_for_machine(db, machine_id)?;
    if endpoints.is_empty() {
        return Ok(CheckOutcome {
            status: "critical".into(),
            message: "No zen endpoints registered for this machine".into(),
            sample: "no endpoints".into(),
            remediation:
                "Run `uecm-cli zen register --host <host> --port 8558` after installing zen on this machine."
                    .into(),
        });
    }

    // Look across all endpoints for any latest-probe reachable inside the
    // freshness window. Same matcher as `core::cache_backend::machine_zen_reachable`
    // but inlined here so we can produce richer outcome messages.
    let conn = db.lock().unwrap();
    let mut any_reachable = false;
    let mut any_probe_seen = false;
    let mut latest_probe_age_secs: Option<i64> = None;
    let mut latest_endpoint_id: Option<i64> = None;
    for ep in &endpoints {
        let Some(endpoint_id) = ep.id else { continue };
        let mut stmt = conn.prepare(
            "SELECT reachable,
                    datetime(probed_at) > datetime('now', ?2) AS fresh,
                    CAST((julianday('now') - julianday(probed_at)) * 86400 AS INTEGER) AS age
             FROM zen_probes
             WHERE endpoint_id = ?1
             ORDER BY datetime(probed_at) DESC, id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![endpoint_id, ZEN_PROBE_FRESHNESS_WINDOW])?;
        if let Some(row) = rows.next()? {
            any_probe_seen = true;
            let reachable: i64 = row.get(0)?;
            let fresh: i64 = row.get(1)?;
            let age_secs: Option<i64> = row.get(2)?;
            // Track most recent across endpoints by smallest age.
            if latest_probe_age_secs.map_or(true, |cur| {
                age_secs.map_or(false, |a| a < cur)
            }) {
                if let Some(a) = age_secs {
                    latest_probe_age_secs = Some(a);
                    latest_endpoint_id = Some(endpoint_id);
                }
            }
            if reachable != 0 && fresh != 0 {
                any_reachable = true;
            }
        }
    }
    drop(conn);

    if any_reachable {
        return Ok(CheckOutcome {
            status: "healthy".into(),
            message: format!(
                "At least one endpoint probed reachable within {}",
                ZEN_PROBE_FRESHNESS_WINDOW.trim_start_matches('-')
            ),
            sample: latest_endpoint_id
                .map(|id| format!("endpoint #{}", id))
                .unwrap_or_default(),
            remediation: String::new(),
        });
    }
    if !any_probe_seen {
        return Ok(CheckOutcome {
            status: "critical".into(),
            message: "No zen probes have ever been recorded for this machine's endpoints".into(),
            sample: format!("{} endpoint(s)", endpoints.len()),
            remediation: "Run `uecm-cli zen probe <endpoint>` to record a baseline probe."
                .into(),
        });
    }
    Ok(CheckOutcome {
        status: "critical".into(),
        message: format!(
            "No reachable probe within {} (last age {}s)",
            ZEN_PROBE_FRESHNESS_WINDOW.trim_start_matches('-'),
            latest_probe_age_secs.unwrap_or(0)
        ),
        sample: latest_endpoint_id
            .map(|id| format!("endpoint #{}", id))
            .unwrap_or_default(),
        remediation: "Re-probe the endpoint; if still unreachable, check the zen service / firewall on the host."
            .into(),
    })
}

fn check_zen_version_consistent(
    db: &Db,
    machine_id: i64,
) -> UecmResult<CheckOutcome> {
    let installs = machine_zen_install::list(db)?;
    let Some(current) = installs.iter().find(|i| i.machine_id == machine_id) else {
        return Ok(CheckOutcome {
            status: "unknown".into(),
            message: "No zen install record for this machine".into(),
            sample: "no install".into(),
            remediation: "Run `uecm-cli zen detect-binary` against this machine to record the install version.".into(),
        });
    };
    let Some(current_version) = current.zenserver_build_version.clone() else {
        return Ok(CheckOutcome {
            status: "unknown".into(),
            message: "No zenserver_build_version recorded for this machine".into(),
            sample: "no version".into(),
            remediation: "Run `uecm-cli zen detect-binary` to record the install version.".into(),
        });
    };

    // Versions across the cluster (only counting machines that actually
    // have a recorded version — None entries get dropped).
    let cluster: Vec<(i64, String)> = installs
        .iter()
        .filter_map(|i| {
            i.zenserver_build_version
                .clone()
                .map(|v| (i.machine_id, v))
        })
        .collect();
    if cluster.len() < 3 {
        return Ok(CheckOutcome {
            status: "healthy".into(),
            message: format!(
                "Cluster size {} below majority threshold (3); skipping consistency check",
                cluster.len()
            ),
            sample: current_version.clone(),
            remediation: String::new(),
        });
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for (_, v) in &cluster {
        *counts.entry(v.as_str()).or_insert(0) += 1;
    }
    let mut ranked: Vec<(&&str, &usize)> = counts.iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(a.1));
    let top = ranked[0];
    // True majority means strictly more than half of the cluster — a
    // plurality (e.g. 2 of {2,1,1}) isn't enough to mark the rest as
    // outliers. Codex P2 caught this: in a `A,A,B,C` cluster the previous
    // `top > second` check would call A the majority even though A has only
    // half the votes.
    let strict_majority = *top.1 * 2 > cluster.len();
    if !strict_majority {
        // Cluster split (e.g. 2-2). Health contract is "everyone on the
        // same build", so a tie still means mixed zen versions — warn
        // everyone in the split, not just one machine.
        return Ok(CheckOutcome {
            status: "warning".into(),
            message: format!(
                "Cluster split across {} version(s) with no clear majority",
                counts.len()
            ),
            sample: current_version,
            remediation: "Pick one zen build and align every machine in the cluster.".into(),
        });
    }
    let majority: &str = *top.0;
    if current_version.as_str() == majority {
        return Ok(CheckOutcome {
            status: "healthy".into(),
            message: format!("Matches cluster majority ({})", majority),
            sample: current_version,
            remediation: String::new(),
        });
    }
    Ok(CheckOutcome {
        status: "warning".into(),
        message: format!(
            "Version {} differs from cluster majority {}",
            current_version, majority
        ),
        sample: current_version,
        remediation: format!(
            "Re-sync this machine to zen build {} or upgrade the rest of the cluster.",
            majority
        ),
    })
}

fn check_zen_binary_intact(db: &Db, machine_id: i64) -> UecmResult<CheckOutcome> {
    let Some(install) = machine_zen_install::find(db, machine_id)? else {
        return Ok(CheckOutcome {
            status: "unknown".into(),
            message: "No zen install record for this machine".into(),
            sample: "no install".into(),
            remediation: "Run `uecm-cli zen detect-binary` against this machine.".into(),
        });
    };
    let (Some(version), Some(actual)) = (
        install.zenserver_build_version.clone(),
        install.zenserver_sha256.clone(),
    ) else {
        return Ok(CheckOutcome {
            status: "unknown".into(),
            message: "Missing zenserver build_version or sha256 in install record".into(),
            sample: "incomplete".into(),
            remediation: "Re-run `uecm-cli zen detect-binary` to capture build_version + sha256."
                .into(),
        });
    };
    // Baseline is keyed by (zen_build_version, binary_kind="zenserver").
    let baseline = zen_binary_expected::find(db, &version, "zenserver")?;
    let Some(expected) = baseline else {
        return Ok(CheckOutcome {
            status: "unknown".into(),
            message: format!(
                "No baseline recorded for zenserver build {}",
                version
            ),
            sample: actual,
            remediation: format!(
                "Record a baseline: `uecm-cli zen baseline insert --build {} --kind zenserver --sha256 <hash>`.",
                version
            ),
        });
    };
    if actual.eq_ignore_ascii_case(&expected.sha256) {
        return Ok(CheckOutcome {
            status: "healthy".into(),
            message: format!("zenserver.exe sha256 matches baseline for {}", version),
            sample: actual,
            remediation: String::new(),
        });
    }
    Ok(CheckOutcome {
        status: "warning".into(),
        message: format!(
            "zenserver.exe sha256 ({}) differs from baseline ({})",
            short_hash(&actual),
            short_hash(&expected.sha256)
        ),
        sample: actual,
        remediation: "Re-sync zenserver.exe from the canonical install source on this machine.".into(),
    })
}

fn short_hash(s: &str) -> String {
    if s.len() <= 12 {
        s.to_string()
    } else {
        format!("{}…", &s[..12])
    }
}

/// Window for which a `zen_cache_stats` sample is considered "fresh enough" to
/// vouch for current `z$` availability. The `core::zen::cache_stats` collector
/// **only** persists rows when it sees `z$` in `/stats` (other providers are
/// tracked but not stored). That means a once-healthy machine whose `z$`
/// quietly disappeared would keep its last-known healthy row in the DB
/// forever; the only signal of degradation is that NO new row arrives. We
/// guard against that by requiring the latest stored row to be within this
/// window — anything older falls back to "stale" status.
///
/// 1 hour is generous (operators don't necessarily run cache-stats every
/// minute), but tight enough that a stuck CI / scheduler is caught well
/// before the next ops review.
pub const ZEN_CACHE_STATS_FRESHNESS_WINDOW: &str = "-1 hour";

fn check_zen_cache_provider_ready(
    db: &Db,
    machine_id: i64,
) -> UecmResult<CheckOutcome> {
    let endpoints = zen_endpoints::list_for_machine(db, machine_id)?;
    if endpoints.is_empty() {
        return Ok(CheckOutcome {
            status: "warning".into(),
            message: "No zen endpoints registered for this machine".into(),
            sample: "no endpoints".into(),
            remediation: "Run `uecm-cli zen register` then `uecm-cli zen cache-stats` to record provider info.".into(),
        });
    }
    // Find the latest cache-stats row across all this machine's endpoints
    // in a single SQL pass so the cross-endpoint comparison is done via
    // `datetime()` rather than raw string compare — string compare gets
    // ISO 'T' vs space-format timestamps wrong (Codex P2).
    let endpoint_ids: Vec<i64> = endpoints.iter().filter_map(|e| e.id).collect();
    if endpoint_ids.is_empty() {
        return Ok(CheckOutcome {
            status: "warning".into(),
            message: "No zen endpoints registered for this machine".into(),
            sample: "no endpoints".into(),
            remediation: "Run `uecm-cli zen register` then `uecm-cli zen cache-stats` to record provider info.".into(),
        });
    }
    let placeholders: Vec<String> = (0..endpoint_ids.len()).map(|i| format!("?{}", i + 2)).collect();
    let sql = format!(
        "SELECT provider_path,
                datetime(sampled_at) > datetime('now', ?1) AS fresh
         FROM zen_cache_stats
         WHERE endpoint_id IN ({})
         ORDER BY datetime(sampled_at) DESC, id DESC
         LIMIT 1",
        placeholders.join(",")
    );
    let latest: Option<(String, bool)> = {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(endpoint_ids.len() + 1);
        params_vec.push(&ZEN_CACHE_STATS_FRESHNESS_WINDOW);
        for id in &endpoint_ids {
            params_vec.push(id);
        }
        let mut rows = stmt.query(rusqlite::params_from_iter(params_vec.iter()))?;
        if let Some(row) = rows.next()? {
            let provider_path: String = row.get(0)?;
            let fresh: i64 = row.get(1)?;
            Some((provider_path, fresh != 0))
        } else {
            None
        }
    };
    let any_seen = latest.is_some();
    if !any_seen {
        return Ok(CheckOutcome {
            status: "warning".into(),
            message: "No zen cache-stats sample recorded for this machine".into(),
            sample: format!("{} endpoint(s)", endpoints.len()),
            remediation: "Run `uecm-cli zen cache-stats` to record provider info.".into(),
        });
    }
    let (provider_path, fresh) = latest.unwrap();
    if !fresh {
        // Stale sample: even if the last sample said z$ was present, we can't
        // claim it still is. The collector ONLY writes rows when z$ shows up,
        // so absence-of-new-rows is the only signal a previously-healthy
        // endpoint can give us. Treat that as "stale".
        return Ok(CheckOutcome {
            status: "warning".into(),
            message: format!(
                "Latest cache-stats sample is stale (older than {})",
                ZEN_CACHE_STATS_FRESHNESS_WINDOW.trim_start_matches('-')
            ),
            sample: provider_path,
            remediation: "Re-run `uecm-cli zen cache-stats` to refresh provider info.".into(),
        });
    }
    // The provider list should mention `z$` (UE-side DDC namespace). The
    // `data::zen_cache_stats::ZenCacheStats.provider_path` field stores the
    // path the cache_stats module recorded — e.g. `/stats/z$`. We just look
    // for `z$` anywhere in that string.
    if provider_path.contains("z$") {
        return Ok(CheckOutcome {
            status: "healthy".into(),
            message: format!("Provider list includes z$ ({})", provider_path),
            sample: provider_path,
            remediation: String::new(),
        });
    }
    Ok(CheckOutcome {
        status: "warning".into(),
        message: format!(
            "Latest provider sample does not advertise z$ ({})",
            provider_path
        ),
        sample: provider_path,
        remediation: "Re-run `uecm-cli zen cache-stats`; if z$ is still missing, confirm zen has the DDC namespace enabled.".into(),
    })
}

fn port_outcome(label: &str, open: bool, fix_hint: &str) -> CheckOutcome {
    if open {
        CheckOutcome {
            status: "healthy".into(),
            message: format!("{} reachable", label),
            sample: "open".into(),
            remediation: String::new(),
        }
    } else {
        CheckOutcome {
            status: "critical".into(),
            message: format!("{} not reachable (TCP connect failed)", label),
            sample: "closed".into(),
            remediation: fix_hint.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::machine_gpus::{GpuInfo, GpuVendor};

    fn gpu(mid: i64, model: &str, drv: &str) -> GpuInfo {
        GpuInfo {
            id: None,
            machine_id: mid,
            gpu_model: model.to_string(),
            driver_version: drv.to_string(),
            vendor: GpuVendor::Nvidia,
            vram_mb: Some(10240),
        }
    }

    #[test]
    fn all_machines_with_same_gpu_are_healthy() {
        let gpus = vec![gpu(1, "RTX 3080", "545.92"), gpu(2, "RTX 3080", "545.92")];
        let report = aggregate_gpu_consistency(&gpus);
        assert_eq!(report.outcomes.get(&1).unwrap().status, "healthy");
        assert_eq!(report.outcomes.get(&2).unwrap().status, "healthy");
    }

    #[test]
    fn one_machine_with_different_driver_is_warning() {
        let gpus = vec![gpu(1, "RTX 3080", "545.92"), gpu(2, "RTX 3080", "537.00")];
        let report = aggregate_gpu_consistency(&gpus);
        assert_eq!(report.outcomes.get(&2).unwrap().status, "warning");
    }

    #[test]
    fn one_machine_with_different_model_is_critical() {
        let gpus = vec![gpu(1, "RTX 3080", "545.92"), gpu(2, "RTX 3080", "545.92"), gpu(3, "RTX 4090", "545.92")];
        let report = aggregate_gpu_consistency(&gpus);
        assert_eq!(report.outcomes.get(&3).unwrap().status, "critical");
    }

    #[test]
    fn machine_with_no_gpu_data_is_unknown() {
        let report = aggregate_gpu_consistency(&[]);
        assert!(report.outcomes.is_empty());
    }

    #[test]
    fn check_outcome_serializes_remediation_field() {
        let outcome = CheckOutcome {
            status: "critical".into(),
            message: "LanmanServer stopped".into(),
            sample: "Stopped".into(),
            remediation: "Start the service: Start-Service LanmanServer".into(),
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("\"remediation\":\"Start the service: Start-Service LanmanServer\""));
    }

    #[test]
    fn check_outcome_deserializes_missing_remediation_as_empty() {
        let json = r#"{"status":"healthy","message":"","sample":""}"#;
        let outcome: CheckOutcome = serde_json::from_str(json).unwrap();
        assert_eq!(outcome.remediation, "");
    }

    #[tokio::test]
    async fn probe_tcp_ports_returns_three_outcomes_with_remediation() {
        // Use TEST-NET-3 so probes time out fast and produce "critical" rows.
        let outcomes = probe_tcp_ports("203.0.113.2", 100).await;
        assert!(outcomes.contains_key("tcp_5985"));
        assert!(outcomes.contains_key("tcp_445"));
        assert!(outcomes.contains_key("tcp_135"));
        // Each closed port must carry a non-empty remediation string.
        for key in ["tcp_5985", "tcp_445", "tcp_135"] {
            let o = outcomes.get(key).unwrap();
            if o.status == "critical" {
                assert!(!o.remediation.is_empty(), "{} missing remediation", key);
            }
        }
    }

    // -------- zen_health_for_machine (T4.2 / T4.3) --------------------------

    use crate::data::{
        machine_zen_install::MachineZenInstall,
        machines::Machine,
        open_in_memory,
        schema,
        zen_binary_expected::ZenBinaryExpected,
        zen_cache_stats::ZenCacheStats,
        zen_endpoints::ZenEndpoint,
        zen_probes::ZenProbe,
    };
    use crate::data::{
        machine_zen_install as mzi, machines as machines_data, zen_binary_expected as zbe,
        zen_cache_stats as zcs, zen_endpoints as zes, zen_probes as zps,
    };

    fn zen_test_db() -> Db {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        db
    }

    fn add_machine(db: &Db, hostname: &str, ip: &str) -> i64 {
        machines_data::insert(db, &Machine::new(hostname, ip)).unwrap()
    }

    fn add_endpoint(db: &Db, machine_id: i64, port: i64) -> i64 {
        zes::upsert(
            db,
            &ZenEndpoint {
                id: None,
                machine_id,
                declared_port: port,
                scheme: "http".into(),
                role: "primary".into(),
                upstream_endpoint_id: None,
                data_dir: "C:\\ZenData".into(),
                httpserverclass: "asio".into(),
                lifecycle_mode: "managed".into(),
                created_at: None,
                updated_at: None,
            },
        )
        .unwrap()
    }

    fn make_probe(endpoint_id: i64, at: &str, reachable: bool) -> ZenProbe {
        ZenProbe {
            id: None,
            endpoint_id,
            probed_at: Some(at.into()),
            reachable,
            schema_version: 1,
            effective_port: Some(8558),
            pid: Some(1),
            uptime_seconds: Some(10),
            data_root: Some("C:\\ZenData".into()),
            is_dedicated: Some(true),
            build_version: Some("5.8.10".into()),
            health_info_cb: None,
            health_version_text: None,
            stats_providers_cb: None,
            error_message: None,
        }
    }

    /// Build a SQLite-friendly "N minutes ago" UTC timestamp string in the
    /// same shape `datetime('now', '-N minutes')` produces — `YYYY-MM-DD HH:MM:SS`.
    fn minutes_ago(db: &Db, minutes: i64) -> String {
        let conn = db.lock().unwrap();
        conn.query_row(
            "SELECT datetime('now', ?1)",
            params![format!("-{} minutes", minutes)],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn zen_reachable_healthy_when_recent_reachable_probe_exists() {
        let db = zen_test_db();
        let mid = add_machine(&db, "RENDER-01", "192.168.10.21");
        let eid = add_endpoint(&db, mid, 8558);
        let when = minutes_ago(&db, 1);
        zps::insert(&db, &make_probe(eid, &when, true)).unwrap();
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        assert_eq!(outcomes.get("zen_reachable").unwrap().status, "healthy");
    }

    #[test]
    fn zen_reachable_critical_when_probe_is_stale() {
        let db = zen_test_db();
        let mid = add_machine(&db, "RENDER-01", "192.168.10.21");
        let eid = add_endpoint(&db, mid, 8558);
        let when = minutes_ago(&db, 30); // outside 5-minute window
        zps::insert(&db, &make_probe(eid, &when, true)).unwrap();
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        let o = outcomes.get("zen_reachable").unwrap();
        assert_eq!(o.status, "critical");
        assert!(!o.remediation.is_empty());
    }

    #[test]
    fn zen_reachable_critical_when_never_probed() {
        let db = zen_test_db();
        let mid = add_machine(&db, "RENDER-01", "192.168.10.21");
        add_endpoint(&db, mid, 8558);
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        let o = outcomes.get("zen_reachable").unwrap();
        assert_eq!(o.status, "critical");
        assert!(o.message.to_lowercase().contains("no zen probes"));
    }

    #[test]
    fn zen_reachable_critical_when_no_endpoints_registered() {
        let db = zen_test_db();
        let mid = add_machine(&db, "RENDER-01", "192.168.10.21");
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        let o = outcomes.get("zen_reachable").unwrap();
        assert_eq!(o.status, "critical");
        assert!(o.remediation.contains("register"));
    }

    fn add_install(db: &Db, machine_id: i64, version: &str, sha: &str) {
        mzi::upsert(
            db,
            &MachineZenInstall {
                machine_id,
                install_dir: Some("C:\\Tools\\Zen".into()),
                zen_cli_path: Some("C:\\Tools\\Zen\\zen.exe".into()),
                zen_cli_build_version: Some(version.into()),
                zen_cli_sha256: Some(sha.into()),
                zenserver_path: Some("C:\\Tools\\Zen\\zenserver.exe".into()),
                zenserver_build_version: Some(version.into()),
                zenserver_sha256: Some(sha.into()),
                last_detected_at: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn zen_version_consistent_healthy_when_three_machines_agree() {
        let db = zen_test_db();
        let m1 = add_machine(&db, "R-01", "10.0.0.1");
        let m2 = add_machine(&db, "R-02", "10.0.0.2");
        let m3 = add_machine(&db, "R-03", "10.0.0.3");
        add_install(&db, m1, "5.8.10", "aaaa");
        add_install(&db, m2, "5.8.10", "aaaa");
        add_install(&db, m3, "5.8.10", "aaaa");
        let outcomes = zen_health_for_machine(&db, m1).unwrap();
        assert_eq!(outcomes.get("zen_version_consistent").unwrap().status, "healthy");
    }

    #[test]
    fn zen_version_consistent_warns_on_outlier() {
        let db = zen_test_db();
        let m1 = add_machine(&db, "R-01", "10.0.0.1");
        let m2 = add_machine(&db, "R-02", "10.0.0.2");
        let m3 = add_machine(&db, "R-03", "10.0.0.3");
        add_install(&db, m1, "5.8.10", "aaaa");
        add_install(&db, m2, "5.8.10", "aaaa");
        add_install(&db, m3, "5.8.9", "bbbb");
        let outcomes = zen_health_for_machine(&db, m3).unwrap();
        let o = outcomes.get("zen_version_consistent").unwrap();
        assert_eq!(o.status, "warning");
        assert!(o.remediation.contains("5.8.10"));

        // Majority member stays healthy.
        let majority = zen_health_for_machine(&db, m1).unwrap();
        assert_eq!(
            majority.get("zen_version_consistent").unwrap().status,
            "healthy"
        );
    }

    #[test]
    fn zen_version_consistent_skips_with_less_than_three_machines() {
        let db = zen_test_db();
        let m1 = add_machine(&db, "R-01", "10.0.0.1");
        let m2 = add_machine(&db, "R-02", "10.0.0.2");
        add_install(&db, m1, "5.8.10", "aaaa");
        add_install(&db, m2, "5.8.9", "bbbb");
        let outcomes = zen_health_for_machine(&db, m1).unwrap();
        assert_eq!(
            outcomes.get("zen_version_consistent").unwrap().status,
            "healthy"
        );
    }

    #[test]
    fn zen_binary_intact_healthy_when_install_sha_matches_baseline() {
        let db = zen_test_db();
        let mid = add_machine(&db, "R-01", "10.0.0.1");
        add_install(&db, mid, "5.8.10", "aaaa");
        zbe::insert_baseline(
            &db,
            &ZenBinaryExpected {
                zen_build_version: "5.8.10".into(),
                binary_kind: "zenserver".into(),
                sha256: "aaaa".into(),
                locked_by: None,
                first_seen_at: None,
            },
        )
        .unwrap();
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        assert_eq!(outcomes.get("zen_binary_intact").unwrap().status, "healthy");
    }

    #[test]
    fn zen_binary_intact_warns_on_install_sha_mismatch() {
        let db = zen_test_db();
        let mid = add_machine(&db, "R-01", "10.0.0.1");
        add_install(&db, mid, "5.8.10", "DRIFTED");
        zbe::insert_baseline(
            &db,
            &ZenBinaryExpected {
                zen_build_version: "5.8.10".into(),
                binary_kind: "zenserver".into(),
                sha256: "BASELINE".into(),
                locked_by: None,
                first_seen_at: None,
            },
        )
        .unwrap();
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        let o = outcomes.get("zen_binary_intact").unwrap();
        assert_eq!(o.status, "warning");
        assert!(o.remediation.to_lowercase().contains("re-sync"));
    }

    #[test]
    fn zen_binary_intact_unknown_when_no_baseline_recorded() {
        // InTree drift is explicitly *not* surfaced — the plan says log only.
        // We exercise the install-path side here; no baseline → "unknown" with
        // a hint, never a finding.
        let db = zen_test_db();
        let mid = add_machine(&db, "R-01", "10.0.0.1");
        add_install(&db, mid, "5.8.10", "aaaa");
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        let o = outcomes.get("zen_binary_intact").unwrap();
        assert_eq!(o.status, "unknown");
        assert!(o.remediation.contains("baseline"));
    }

    fn add_cache_stats(db: &Db, endpoint_id: i64, provider_path: &str, at: &str) {
        zcs::insert(
            db,
            &ZenCacheStats {
                id: None,
                endpoint_id,
                sampled_at: Some(at.into()),
                cache_hit_ratio: Some(0.5),
                cache_disk_size_bytes: Some(1_000),
                cache_memory_size_bytes: Some(100),
                provider_path: provider_path.into(),
                raw_cb: vec![],
                schema_version: 1,
            },
        )
        .unwrap();
    }

    #[test]
    fn zen_cache_provider_ready_healthy_when_z_dollar_present() {
        let db = zen_test_db();
        let mid = add_machine(&db, "R-01", "10.0.0.1");
        let eid = add_endpoint(&db, mid, 8558);
        // Use a current-time anchored sample so it's inside the freshness window.
        let now = minutes_ago(&db, 0);
        add_cache_stats(&db, eid, "/stats/z$", &now);
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        assert_eq!(
            outcomes.get("zen_cache_provider_ready").unwrap().status,
            "healthy"
        );
    }

    #[test]
    fn zen_cache_provider_ready_warns_when_provider_lacks_z_dollar() {
        let db = zen_test_db();
        let mid = add_machine(&db, "R-01", "10.0.0.1");
        let eid = add_endpoint(&db, mid, 8558);
        let now = minutes_ago(&db, 0);
        add_cache_stats(&db, eid, "/stats/other", &now);
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        let o = outcomes.get("zen_cache_provider_ready").unwrap();
        assert_eq!(o.status, "warning");
        assert!(!o.remediation.is_empty());
    }

    #[test]
    fn zen_cache_provider_ready_picks_latest_across_mixed_timestamp_formats() {
        // Codex P2: cache_stats can store both `YYYY-MM-DD HH:MM:SS` (from
        // CURRENT_TIMESTAMP) and ISO `YYYY-MM-DDTHH:MM:SSZ` (from explicit
        // inserts). String comparison gets that wrong; SQL `datetime()`
        // ordering must be authoritative.
        let db = zen_test_db();
        let mid = add_machine(&db, "R-01", "10.0.0.1");
        let eid = add_endpoint(&db, mid, 8558);
        // Newer ISO sample with z$ that lexicographically sorts AFTER a
        // space-format same-day sample (T > space in ASCII).
        let now_space = minutes_ago(&db, 0);
        let one_minute_ago_iso = {
            let conn = db.lock().unwrap();
            // ISO format anchored to "now - 1 min" so it's strictly older
            // than `now_space` chronologically; despite that, the literal
            // string has `T` > ` ` and would win a naive string compare.
            conn.query_row(
                "SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 minutes')",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
        };
        // Insert ISO row FIRST (older chronologically) then space row
        // (newer chronologically). Naive string compare would pick the ISO
        // row as latest because `T` > ` `.
        add_cache_stats(&db, eid, "/stats/other", &one_minute_ago_iso);
        add_cache_stats(&db, eid, "/stats/z$", &now_space);
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        // The actually-latest row is the space one with z$ → healthy.
        assert_eq!(
            outcomes.get("zen_cache_provider_ready").unwrap().status,
            "healthy",
            "datetime() ordering must beat lexicographic, got: {:?}",
            outcomes.get("zen_cache_provider_ready")
        );
    }

    #[test]
    fn zen_version_consistent_warns_on_plurality_not_majority() {
        // Codex P2: `A,A,B,C` is not a strict majority for A. Both A
        // machines should warn (not be greened) and the rest should too.
        let db = zen_test_db();
        let m1 = add_machine(&db, "R-01", "10.0.0.1");
        let m2 = add_machine(&db, "R-02", "10.0.0.2");
        let m3 = add_machine(&db, "R-03", "10.0.0.3");
        let m4 = add_machine(&db, "R-04", "10.0.0.4");
        add_install(&db, m1, "5.8.10", "aa");
        add_install(&db, m2, "5.8.10", "aa");
        add_install(&db, m3, "5.8.9", "bb");
        add_install(&db, m4, "5.8.8", "cc");
        for mid in [m1, m2, m3, m4] {
            let outcomes = zen_health_for_machine(&db, mid).unwrap();
            assert_eq!(
                outcomes.get("zen_version_consistent").unwrap().status,
                "warning",
                "machine {} should warn under plurality",
                mid
            );
        }
    }

    #[test]
    fn zen_cache_provider_ready_warns_when_sample_is_stale() {
        // Codex P2: previously-healthy machine whose `z$` disappears never
        // gets a new row inserted (collector only writes when `z$` is
        // present). The latest row in DB is forever the old healthy one.
        // Recency gate must catch this.
        let db = zen_test_db();
        let mid = add_machine(&db, "R-01", "10.0.0.1");
        let eid = add_endpoint(&db, mid, 8558);
        // 6 hours ago — well outside the 1-hour freshness window.
        let stale = {
            let conn = db.lock().unwrap();
            conn.query_row(
                "SELECT datetime('now', '-6 hours')",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
        };
        add_cache_stats(&db, eid, "/stats/z$", &stale);
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        let o = outcomes.get("zen_cache_provider_ready").unwrap();
        assert_eq!(o.status, "warning");
        assert!(
            o.message.to_lowercase().contains("stale"),
            "expected 'stale' in message, got: {}",
            o.message
        );
    }

    #[test]
    fn zen_cache_provider_ready_warns_when_no_stats_recorded() {
        let db = zen_test_db();
        let mid = add_machine(&db, "R-01", "10.0.0.1");
        add_endpoint(&db, mid, 8558);
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        assert_eq!(
            outcomes.get("zen_cache_provider_ready").unwrap().status,
            "warning"
        );
    }

    #[test]
    fn zen_health_for_machine_returns_all_four_rows() {
        // Sanity: every key must always be present so callers can rely on
        // a stable shape.
        let db = zen_test_db();
        let mid = add_machine(&db, "R-01", "10.0.0.1");
        let outcomes = zen_health_for_machine(&db, mid).unwrap();
        for key in [
            "zen_reachable",
            "zen_version_consistent",
            "zen_binary_intact",
            "zen_cache_provider_ready",
        ] {
            assert!(outcomes.contains_key(key), "missing key {}", key);
        }
    }
}
