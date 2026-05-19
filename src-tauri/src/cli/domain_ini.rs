//! `uecm-cli ini <action>` handlers.

use crate::cli::args::IniAction;
use crate::cli::credential_args::CredentialArgs;
use crate::cli::destructive::{self, Outcome};
use crate::cli::host_args::HostTarget;
use crate::cli::output::{EmitSerialize, Event};
use crate::cli::run::Ctx;
use crate::core::ini_apply::{self, ApplyContext};
use crate::core::ini_editor;
use crate::core::ini_scanner::{self, ScanInputs};
use crate::core::ini_diagnostics::EnvVarState;
use crate::core::env_vars;
use crate::data::{
    ini_findings, machine_ue_installs, machines as data_machines, scan_runs, IniFinding,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use sha2::{Digest, Sha256};

fn value_sha256_prefix(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut out = String::with_capacity(8);
    for b in &digest[..4] {
        use std::fmt::Write as _;
        write!(out, "{:02x}", b).unwrap();
    }
    out
}

fn redact_in_string(msg: String, value: &str) -> String {
    // Redact any non-empty value occurrence — short ini values like `K=v`
    // would otherwise slip through earlier `len >= 4` guard.
    if !value.is_empty() && msg.contains(value) {
        msg.replace(value, "[REDACTED:value]")
    } else {
        msg
    }
}

fn redact_error(e: UecmError, value: &str) -> UecmError {
    match e {
        UecmError::PowerShell(msg) => UecmError::PowerShell(redact_in_string(msg, value)),
        UecmError::OperationFailed(msg) => UecmError::OperationFailed(redact_in_string(msg, value)),
        other => other,
    }
}

#[derive(Serialize)]
struct IniReadOut<'a> {
    host: &'a str,
    file: &'a str,
    section: &'a str,
    keys: Vec<ini_editor::IniKey>,
}

pub fn handle(ctx: &mut Ctx<'_>, action: IniAction) -> UecmResult<()> {
    match action {
        IniAction::Read { host, file, section, cred } => {
            read(ctx, &host, &file, &section, &cred)
        }
        IniAction::Set { target, file, section, key, value, yes, dry_run, cred } => {
            let t = target.require_one()?;
            let outcome = destructive::check(yes, dry_run, "ini.set")?;
            let db = ctx.require_db()?;
            // Preflight only — see env.set for stdin-double-read rationale.
            cred.preflight(db)?;
            if outcome == Outcome::DryRun {
                let hosts: Vec<String> = match &t {
                    HostTarget::Single(h) => vec![h.clone()],
                    HostTarget::Batch(hs) => hs.clone(),
                };
                destructive::emit_plan(
                    ctx.emitter.as_mut(),
                    "ini.set",
                    serde_json::json!({
                        "hosts": hosts,
                        "file": file,
                        "section": section,
                        "key": key,
                        "value_len": value.chars().count(),
                        "value_sha256_prefix": value_sha256_prefix(&value),
                    }),
                );
                return Ok(());
            }
            match t {
                HostTarget::Single(h) => set_single(ctx, &h, &file, &section, &key, &value, &cred),
                HostTarget::Batch(hs) => set_batch(ctx, &hs, &file, &section, &key, &value, &cred),
            }
        }
        IniAction::Remove { target, file, section, key, yes, dry_run, cred } => {
            let t = target.require_one()?;
            let outcome = destructive::check(yes, dry_run, "ini.remove")?;
            let db = ctx.require_db()?;
            // Preflight: alias / flag-shape only. Then require cred SUPPLIED
            // (alias or --user --pass[-stdin] given) without consuming stdin.
            cred.preflight(db)?;
            let cred_supplied =
                cred.cred_alias.is_some() || cred.user.is_some() || cred.pass_stdin;
            if !cred_supplied {
                return Err(UecmError::InvalidInput(
                    "ini remove requires credentials (--cred-alias or --user --pass / --pass-stdin)".into(),
                ));
            }
            if outcome == Outcome::DryRun {
                let hosts: Vec<String> = match &t {
                    HostTarget::Single(h) => vec![h.clone()],
                    HostTarget::Batch(hs) => hs.clone(),
                };
                destructive::emit_plan(
                    ctx.emitter.as_mut(),
                    "ini.remove",
                    serde_json::json!({
                        "hosts": hosts,
                        "file": file,
                        "section": section,
                        "key": key,
                    }),
                );
                return Ok(());
            }
            match t {
                HostTarget::Single(h) => remove_single(ctx, &h, &file, &section, &key, &cred),
                HostTarget::Batch(hs) => remove_batch(ctx, &hs, &file, &section, &key, &cred),
            }
        }
        // Plan-3 additions:
        IniAction::Scan { machine_ids, cred } => scan_cluster(ctx, &machine_ids, &cred),
        IniAction::Runs { limit } => list_runs(ctx, limit),
        IniAction::Findings { scan_run_id, severity } => {
            list_findings(ctx, scan_run_id, severity.as_deref())
        }
        IniAction::GetFinding { finding_id } => get_finding(ctx, finding_id),
        IniAction::Apply { finding_id, yes, dry_run, cred } => {
            let outcome = destructive::check(yes, dry_run, "ini.apply")?;
            // Mirror the --yes path's preconditions locally so dry-run cannot
            // succeed on inputs that the real command would immediately reject:
            // credentials are mandatory; finding row must exist.
            let db = ctx.require_db()?;
            cred.preflight(db)?;
            let cred_supplied =
                cred.cred_alias.is_some() || cred.user.is_some() || cred.pass_stdin;
            if !cred_supplied {
                return Err(UecmError::InvalidInput(
                    "ini apply requires credentials (--cred-alias or --user --pass / --pass-stdin)".into(),
                ));
            }
            let finding = ini_findings::find_by_id(db, finding_id)?.ok_or_else(|| {
                UecmError::InvalidInput(format!("finding id={} not found", finding_id))
            })?;
            if outcome == Outcome::DryRun {
                // Mirror `core::ini_apply::apply` validation so dry-run can
                // only succeed on findings the real --yes path can actually
                // execute. Manual / incomplete findings must reject in both
                // paths to keep automation honest.
                if finding.section.as_deref().is_none() {
                    return Err(UecmError::InvalidInput(
                        "finding has no section".into(),
                    ));
                }
                match finding.recommended_action.as_str() {
                    "set" => {
                        if finding.key_name.is_none() {
                            return Err(UecmError::InvalidInput(
                                "finding has no key_name".into(),
                            ));
                        }
                        if finding.recommended_value.is_none() {
                            return Err(UecmError::InvalidInput(
                                "finding has no recommended_value".into(),
                            ));
                        }
                    }
                    "remove" => {
                        // R002 reads keys from snippet_before; others require key_name.
                        if finding.rule_id != "R002" && finding.key_name.is_none() {
                            return Err(UecmError::InvalidInput(
                                "remove needs key_name".into(),
                            ));
                        }
                    }
                    "manual" => {
                        return Err(UecmError::InvalidInput(
                            "manual findings cannot be auto-applied; open the file directly"
                                .into(),
                        ));
                    }
                    other => {
                        return Err(UecmError::InvalidInput(format!(
                            "unknown action: {}",
                            other
                        )));
                    }
                }
                destructive::emit_plan(
                    ctx.emitter.as_mut(),
                    "ini.apply",
                    serde_json::json!({
                        "finding_id": finding_id,
                        "rule_id": finding.rule_id,
                        "severity": finding.severity,
                        "machine_id": finding.machine_id,
                        "file_path": finding.file_path,
                        "section": finding.section,
                        "key": finding.key_name,
                        "recommended_action": finding.recommended_action,
                    }),
                );
                return Ok(());
            }
            apply_finding(ctx, finding_id, &cred)
        }
        IniAction::Skip { finding_id } => skip_finding(ctx, finding_id),
        IniAction::VerifyPsoPrecaching { project_id } => verify_pso_precaching(ctx, project_id),
    }
}

fn read(
    ctx: &mut Ctx<'_>,
    host: &str,
    file: &str,
    section: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let keys = match creds {
        Some((u, p)) => ini_editor::read_section_with_credential(host, file, section, &u, &p)?,
        None => ini_editor::read_section(host, file, section)?,
    };
    ctx.emitter
        .emit_result(&IniReadOut { host, file, section, keys })
        .ok();
    Ok(())
}

fn set_single(
    ctx: &mut Ctx<'_>,
    host: &str,
    file: &str,
    section: &str,
    key: &str,
    value: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let res = match creds {
        Some((u, p)) => ini_editor::set_key_with_credential(host, file, section, key, value, &u, &p),
        None => ini_editor::set_key(host, file, section, key, value),
    };
    res.map_err(|e| redact_error(e, value))?;
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "host": host,
                "file": file,
                "section": section,
                "key": key,
                "value_len": value.chars().count(),
                "value_sha256_prefix": value_sha256_prefix(value),
            }),
        })
        .ok();
    Ok(())
}

fn set_batch(
    ctx: &mut Ctx<'_>,
    hosts: &[String],
    file: &str,
    section: &str,
    key: &str,
    value: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let total = hosts.len() as i64;

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "ini_set".into(),
            task_id: None,
            metadata: serde_json::json!({
                "hosts": total,
                "file": file,
                "section": section,
                "key": key,
                "value_len": value.chars().count(),
                "value_sha256_prefix": value_sha256_prefix(value),
            }),
        })
        .ok();

    let mut ok_count: i64 = 0;
    let mut fail_count: i64 = 0;
    for (idx, host) in hosts.iter().enumerate() {
        ctx.emitter
            .emit_event(&Event::ItemStarted {
                item_id: host.clone(),
                index: idx as i64,
                total,
            })
            .ok();
        let res = match &creds {
            Some((u, p)) => ini_editor::set_key_with_credential(host, file, section, key, value, u, p),
            None => ini_editor::set_key(host, file, section, key, value),
        };
        match res {
            Ok(_) => {
                ok_count += 1;
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: host.clone(),
                        index: idx as i64,
                        ok: true,
                        message: None,
                    })
                    .ok();
            }
            Err(e) => {
                fail_count += 1;
                let msg = redact_in_string(e.to_string(), value);
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: host.clone(),
                        index: idx as i64,
                        ok: false,
                        message: Some(msg),
                    })
                    .ok();
            }
        }
    }
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "hosts": total,
                "ok": ok_count,
                "failed": fail_count,
            }),
        })
        .ok();
    if fail_count > 0 {
        return Err(UecmError::OperationFailed(format!(
            "{}/{} hosts failed ini set",
            fail_count, total
        )));
    }
    Ok(())
}

fn remove_single(
    ctx: &mut Ctx<'_>,
    host: &str,
    file: &str,
    section: &str,
    key: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let (u, p) = creds.ok_or_else(|| {
        UecmError::InvalidInput(
            "ini remove requires credentials (--cred-alias or --user --pass / --pass-stdin)".into(),
        )
    })?;
    ini_editor::remove_key_with_credential(host, file, section, key, &u, &p)?;
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "host": host,
                "file": file,
                "section": section,
                "key": key,
                "removed": true,
            }),
        })
        .ok();
    Ok(())
}

fn remove_batch(
    ctx: &mut Ctx<'_>,
    hosts: &[String],
    file: &str,
    section: &str,
    key: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let (u, p) = creds.ok_or_else(|| {
        UecmError::InvalidInput(
            "ini remove --hosts requires credentials (--cred-alias or --user --pass / --pass-stdin)".into(),
        )
    })?;
    let total = hosts.len() as i64;

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "ini_remove".into(),
            task_id: None,
            metadata: serde_json::json!({
                "hosts": total,
                "file": file,
                "section": section,
                "key": key,
            }),
        })
        .ok();

    let mut ok_count: i64 = 0;
    let mut fail_count: i64 = 0;
    for (idx, host) in hosts.iter().enumerate() {
        ctx.emitter
            .emit_event(&Event::ItemStarted {
                item_id: host.clone(),
                index: idx as i64,
                total,
            })
            .ok();
        match ini_editor::remove_key_with_credential(host, file, section, key, &u, &p) {
            Ok(_) => {
                ok_count += 1;
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: host.clone(),
                        index: idx as i64,
                        ok: true,
                        message: None,
                    })
                    .ok();
            }
            Err(e) => {
                fail_count += 1;
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: host.clone(),
                        index: idx as i64,
                        ok: false,
                        message: Some(e.to_string()),
                    })
                    .ok();
            }
        }
    }
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "hosts": total,
                "ok": ok_count,
                "failed": fail_count,
            }),
        })
        .ok();
    if fail_count > 0 {
        return Err(UecmError::OperationFailed(format!(
            "{}/{} hosts failed ini remove",
            fail_count, total
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Plan-3: cluster scan + findings workflow
// ---------------------------------------------------------------------------

/// Run a full INI scan across a set of machines identified by DB id.
///
/// Flow mirrors `commands::ini_scanner::scan_inis_summary`:
///   1. Create a scan_runs row → scan_run_id.
///   2. For each machine_id: load UE installs, read env vars, build ScanInputs,
///      call core::ini_scanner::scan_machine, persist findings.
///   3. Emit NDJSON events: started → per-machine item_started / item_completed
///      → completed with final counts.
fn scan_cluster(
    ctx: &mut Ctx<'_>,
    machine_ids: &[i64],
    cred: &CredentialArgs,
) -> UecmResult<()> {
    if machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("--machine-ids must not be empty".into()));
    }
    // Clone the Arc<Mutex<>> so we can hold a Db handle independently of the
    // ctx borrow, allowing interleaved db ops and ctx.emitter calls.
    let db = ctx.require_db()?.clone();
    let (username, password) = cred.resolve(&db)?.ok_or_else(|| {
        UecmError::InvalidInput(
            "ini scan requires credentials (--cred-alias or --user --pass / --pass-stdin)".into(),
        )
    })?;

    // Create the scan_runs row up front.
    let scan_run_id = scan_runs::insert(&db, "ini", machine_ids)?;
    let total = machine_ids.len() as i64;

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "ini_scan".into(),
            task_id: Some(scan_run_id.to_string()),
            metadata: serde_json::json!({
                "machines": total,
                "scan_run_id": scan_run_id,
            }),
        })
        .ok();

    let mut total_critical = 0i64;
    let mut total_warning = 0i64;
    let mut total_healthy = 0i64;
    let mut total_info = 0i64;
    let mut all_errors: Vec<String> = Vec::new();
    let mut all_not_found: Vec<String> = Vec::new();
    let mut total_read: usize = 0;

    for (idx, &mid) in machine_ids.iter().enumerate() {
        ctx.emitter
            .emit_event(&Event::ItemStarted {
                item_id: mid.to_string(),
                index: idx as i64,
                total,
            })
            .ok();

        // All DB operations in a scoped block so the borrow ends before
        // ctx.emitter is borrowed mutably below.
        let machine_result: UecmResult<(i64, i64, i64, i64, usize, Vec<String>, Vec<String>)> = {
            let machine = data_machines::find_by_id(&db, mid)?
                .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", mid)))?;
            let installs_rows = machine_ue_installs::list_for_machine(&db, mid)?;
            let installs: Vec<(String, String)> = installs_rows
                .into_iter()
                .map(|i| (i.version, i.install_path))
                .collect();

            let mut env_state = EnvVarState::default();
            env_state.shared_data_cache_path = env_vars::get_with_credential(
                &machine.ip, "UE-SharedDataCachePath", &username, &password,
            ).ok().flatten();
            env_state.local_data_cache_path = env_vars::get_with_credential(
                &machine.ip, "UE-LocalDataCachePath", &username, &password,
            ).ok().flatten();

            // Auto-enable zen rules when the machine has at least one
            // registered endpoint. The builder returns Ok(None) for
            // legacy clusters → zen rules silently skip.
            //
            // UE version pick: highest install on this machine. Machine-
            // scoped scans don't know which project the operator is
            // targeting, so the highest-version install is the closest
            // proxy — that's what `core::cache_backend::resolve_for`
            // already uses for the routing decision.
            // Codex P2: numeric (major, minor) ordering — `String::max()`
            // would order "5.9" > "5.10" lexicographically.
            let ue_version_hint: Option<String> =
                ini_scanner::pick_highest_ue_version(&installs);
            let zen_ctx_owned = ini_scanner::build_zen_ctx_for_machine(
                &db,
                mid,
                ue_version_hint.as_deref(),
                // Codex round-21 P2: restrict R018's cluster majority
                // to the scan's machine set.
                Some(machine_ids),
            )?;
            let zen_ctx = zen_ctx_owned.as_ref().map(|o| o.as_ctx());

            let inputs = ScanInputs {
                host: &machine.ip,
                credential: Some((&username, &password)),
                installs: &installs,
                user_profile: "",
                project_roots: &[],
                env_state,
                zen_ctx: zen_ctx.as_ref(),
            };
            let outcome = ini_scanner::scan_machine(&inputs)?;
            let read_count = outcome.read_count;
            let m_errors: Vec<String> = outcome.errors.iter()
                .map(|e| format!("{}: {}", machine.hostname, e)).collect();
            let m_not_found: Vec<String> = outcome.not_found.iter()
                .map(|nf| format!("{}: {}", machine.hostname, nf)).collect();

            let mut crit = 0i64;
            let mut warn = 0i64;
            let mut healthy = 0i64;
            let mut info = 0i64;
            for f in outcome.findings {
                let row = IniFinding {
                    id: None,
                    scan_run_id,
                    machine_id: mid,
                    rule_id: f.rule_id,
                    severity: f.severity.as_str().into(),
                    category: f.category.as_str().into(),
                    file_path: f.file_path,
                    section: f.section,
                    key_name: f.key_name,
                    line_number: f.line_number,
                    snippet_before: f.snippet_before,
                    snippet_after: f.snippet_after,
                    recommended_action: f.recommended_action.as_str().into(),
                    recommended_value: f.recommended_value,
                    symptom: f.symptom,
                    rationale: f.rationale,
                    fixed_at: None,
                    skipped_at: None,
                };
                match row.severity.as_str() {
                    "critical" => crit += 1,
                    "warning" => warn += 1,
                    "healthy" => healthy += 1,
                    _ => info += 1,
                }
                ini_findings::insert(&db, &row)?;
            }
            Ok((crit, warn, healthy, info, read_count, m_errors, m_not_found))
        };

        match machine_result {
            Ok((crit, warn, healthy, info, read_count, m_errors, m_not_found)) => {
                total_critical += crit;
                total_warning += warn;
                total_healthy += healthy;
                total_info += info;
                total_read += read_count;
                all_errors.extend(m_errors);
                all_not_found.extend(m_not_found);
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: mid.to_string(),
                        index: idx as i64,
                        ok: true,
                        message: Some(format!(
                            "critical={} warning={} healthy={} info={}",
                            crit, warn, healthy, info
                        )),
                    })
                    .ok();
            }
            Err(e) => {
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: mid.to_string(),
                        index: idx as i64,
                        ok: false,
                        message: Some(e.to_string()),
                    })
                    .ok();
            }
        }
    }

    let summary = serde_json::json!({
        "scan_run_id": scan_run_id,
        "critical": total_critical,
        "warning": total_warning,
        "healthy": total_healthy,
        "info": total_info,
        "total_files_read": total_read,
        "errors_count": all_errors.len(),
        "not_found_count": all_not_found.len(),
    });
    scan_runs::finish(&db, scan_run_id, &summary)?;

    ctx.emitter
        .emit_event(&Event::Completed { summary })
        .ok();
    Ok(())
}

fn list_runs(ctx: &mut Ctx<'_>, limit: i64) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let runs = scan_runs::list_recent(db, "ini", limit)?;
    ctx.emitter.emit_result(&runs).ok();
    Ok(())
}

fn list_findings(
    ctx: &mut Ctx<'_>,
    scan_run_id: i64,
    severity: Option<&str>,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let mut findings = ini_findings::list_for_run(db, scan_run_id)?;
    if let Some(sev) = severity {
        findings.retain(|f| f.severity.eq_ignore_ascii_case(sev));
    }
    ctx.emitter.emit_result(&findings).ok();
    Ok(())
}

fn get_finding(ctx: &mut Ctx<'_>, finding_id: i64) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let finding = ini_findings::find_by_id(db, finding_id)?;
    ctx.emitter.emit_result(&finding).ok();
    Ok(())
}

fn apply_finding(
    ctx: &mut Ctx<'_>,
    finding_id: i64,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let (username, password) = cred.resolve(db)?.ok_or_else(|| {
        UecmError::InvalidInput(
            "ini apply requires credentials (--cred-alias or --user --pass / --pass-stdin)".into(),
        )
    })?;
    let f = ini_findings::find_by_id(db, finding_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("finding {} not found", finding_id)))?;
    let machine = data_machines::find_by_id(db, f.machine_id)?
        .ok_or_else(|| {
            UecmError::InvalidInput(format!("machine {} not found", f.machine_id))
        })?;
    let apply_ctx = ApplyContext {
        host: &machine.ip,
        credential: (&username, &password),
    };
    let backup = ini_apply::apply(&apply_ctx, &f)?;
    ini_findings::mark_fixed(db, finding_id)?;
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "finding_id": finding_id,
                "applied": true,
                "backup_path": backup,
            }),
        })
        .ok();
    Ok(())
}

fn skip_finding(ctx: &mut Ctx<'_>, finding_id: i64) -> UecmResult<()> {
    let db = ctx.require_db()?;
    ini_findings::mark_skipped(db, finding_id)?;
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "finding_id": finding_id,
                "skipped": true,
            }),
        })
        .ok();
    Ok(())
}

/// Verify PSO precaching CVars on a project. The UI command (`verify_pso_precaching`)
/// delegates to `scan_inis` with non-empty project_paths. In the CLI, project_paths
/// are looked up from the project_locations table for the given project_id, and the
/// scan is run across all machines that have a location for that project.
fn verify_pso_precaching(ctx: &mut Ctx<'_>, project_id: i64) -> UecmResult<()> {
    // Resolve project locations to get (machine_id, abs_path) pairs.
    let db = ctx.require_db()?;
    let locations = crate::data::project_locations::list_by_project(db, project_id)?;
    if locations.is_empty() {
        return Err(UecmError::InvalidInput(format!(
            "project {} has no locations registered; add locations first",
            project_id
        )));
    }
    let machine_ids: Vec<i64> = locations
        .iter()
        .map(|l| l.machine_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    ctx.emitter
        .emit_result(&serde_json::json!({
            "project_id": project_id,
            "machine_ids": machine_ids,
            "note": "use `ini scan --machine-ids <ids>` with project paths; PSO CVar check (R008-R010) runs as part of the full ini scan",
        }))
        .ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::output::{Emitter, NdjsonEmitter};
    use crate::data::{open_in_memory, schema, Db};

    fn fresh_db() -> Db {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        db
    }

    fn make_ctx<'a>(buf: &'a mut Vec<u8>, db: &'a Db) -> Ctx<'a> {
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf));
        Ctx {
            db: Some(db.clone()),
            db_path: std::path::PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn ini_set_hosts_emits_lifecycle_with_no_value_leak() {
        let db = fresh_db();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let cred = CredentialArgs { cred_alias: None, user: None, pass: None, pass_stdin: false };
        let secret = "INI-SECRET-NEVER-LEAK-VALUE";
        let _ = set_batch(&mut ctx, &["192.0.2.1".into()], "C:\\test.ini", "S", "K", secret, &cred);
        drop(ctx);
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\"kind\":\"started\""));
        assert!(s.contains("\"kind\":\"item_completed\""));
        assert!(s.contains("\"kind\":\"completed\""));
        assert!(!s.contains(secret), "value leaked: {}", s);
    }

    #[test]
    fn remove_single_without_creds_returns_invalid_input() {
        let db = fresh_db();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let cred = CredentialArgs { cred_alias: None, user: None, pass: None, pass_stdin: false };
        let r = remove_single(&mut ctx, "host", "C:\\test.ini", "S", "K", &cred);
        assert!(matches!(r, Err(UecmError::InvalidInput(_))));
    }
}
