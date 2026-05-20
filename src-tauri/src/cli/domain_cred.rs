//! `uecm-cli cred <action>` handlers.

use crate::cli::args::CredAction;
use crate::cli::destructive::{self, Outcome};
use crate::cli::output::EmitSerialize;
use crate::cli::run::Ctx;
use crate::data::credentials as data_creds;
use crate::error::{UecmError, UecmResult};
use std::io::{self, BufRead};

pub fn handle(ctx: &mut Ctx<'_>, action: CredAction) -> UecmResult<()> {
    match action {
        CredAction::List => list(ctx),
        CredAction::Save {
            alias,
            user,
            pass,
            pass_stdin,
            kind,
        } => save(ctx, &alias, &user, pass.as_deref(), pass_stdin, &kind),
        CredAction::Delete { alias, yes, dry_run } => delete(ctx, &alias, yes, dry_run),
    }
}

fn list(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = data_creds::list_all(db)?;
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}

fn read_password(pass_inline: Option<&str>, pass_stdin: bool) -> UecmResult<String> {
    if let Some(p) = pass_inline {
        return Ok(p.to_string());
    }
    if pass_stdin {
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line).map_err(|e| {
            UecmError::InvalidInput(format!("read password from stdin: {}", e))
        })?;
        return Ok(line.trim_end_matches(['\r', '\n']).to_string());
    }
    Err(UecmError::InvalidInput(
        "either --pass or --pass-stdin is required".into(),
    ))
}

fn save(
    ctx: &mut Ctx<'_>,
    alias: &str,
    user: &str,
    pass_inline: Option<&str>,
    pass_stdin: bool,
    kind: &str,
) -> UecmResult<()> {
    let password = read_password(pass_inline, pass_stdin)?;
    save_resolved(ctx, alias, user, &password, kind)
}

/// Persist an already-resolved `(user, password)` credential under `alias`
/// (cmdkey + DPAPI + SQLite metadata). Shared by `cred save` and by
/// `machine authorize --save-as` — the latter has the password in hand already
/// and must not re-read stdin.
pub(crate) fn save_resolved(
    ctx: &mut Ctx<'_>,
    alias: &str,
    user: &str,
    password: &str,
    kind: &str,
) -> UecmResult<()> {
    use crate::core::credentials as core_creds;

    let username = core_creds::normalize_username_for_storage(user);

    // Validate --kind BEFORE any side effects (cmdkey / DPAPI). A typo here
    // used to silently fall back to Winrm and persist under the wrong type.
    let _validated_kind = parse_credential_kind(kind)?;

    // Step 2: cmdkey first. If this fails, nothing else gets written.
    core_creds::store(alias, &username, password)?;

    // Step 3: DPAPI. If it fails, roll back cmdkey before propagating.
    if let Err(dpapi_err) = core_creds::store_password(alias, password) {
        if let Err(rollback_err) = core_creds::delete(alias) {
            tracing::warn!(
                alias = %alias,
                error = %rollback_err,
                "cmdkey rollback after DPAPI failure also failed"
            );
        }
        return Err(dpapi_err);
    }

    // Step 4+5: SQLite. Replace if alias already exists, else insert.
    let db = ctx.require_db()?;
    if data_creds::find_by_alias(db, alias)?.is_some() {
        data_creds::delete_by_alias(db, alias)?;
    }
    // Build record (kind already validated above, so unwrap is sound).
    let record = build_credential_record(alias, &username, kind)?;
    let id = data_creds::insert(db, &record)?;

    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed {
            summary: serde_json::json!({ "id": id, "alias": alias }),
        })
        .ok();
    Ok(())
}

fn parse_credential_kind(kind_str: &str) -> UecmResult<data_creds::CredentialKind> {
    use data_creds::CredentialKind;
    match kind_str.to_lowercase().as_str() {
        "winrm" => Ok(CredentialKind::Winrm),
        "share" => Ok(CredentialKind::Share),
        other => Err(UecmError::InvalidInput(format!(
            "unknown credential kind '{}'; expected 'winrm' or 'share'",
            other
        ))),
    }
}

fn build_credential_record(
    alias: &str,
    username: &str,
    kind_str: &str,
) -> UecmResult<data_creds::CredentialRecord> {
    use data_creds::CredentialRecord;
    let kind = parse_credential_kind(kind_str)?;
    Ok(CredentialRecord {
        id: None,
        alias: alias.to_string(),
        kind,
        username: username.to_string(),
    })
}

fn delete(ctx: &mut Ctx<'_>, alias: &str, yes: bool, dry_run: bool) -> UecmResult<()> {
    use crate::core::credentials as core_creds;

    let outcome = destructive::check(yes, dry_run, "cred.delete")?;

    let db = ctx.require_db()?;

    if outcome == Outcome::DryRun {
        let exists = data_creds::find_by_alias(db, alias)?.is_some();
        destructive::emit_plan(
            ctx.emitter.as_mut(),
            "cred.delete",
            serde_json::json!({
                "alias": alias,
                "exists_in_db": exists,
                "side_effects": ["cmdkey delete", "SQLite delete", "DPAPI delete (best-effort)"],
            }),
        );
        return Ok(());
    }

    // Step 1: cmdkey delete — keep result, propagate at the end.
    let cm_result = core_creds::delete(alias);

    // Step 2: SQLite delete — environment error if this fails, propagate now.
    data_creds::delete_by_alias(db, alias)?;

    // Step 3: DPAPI best-effort.
    if let Err(e) = core_creds::delete_password(alias) {
        tracing::warn!(
            alias = %alias,
            error = %e,
            "DPAPI delete_password failed; orphan entry will remain in creds.bin"
        );
    }

    // Step 4: surface cmdkey result.
    cm_result.map(|_| {
        let _ = ctx.emitter.emit_event(&crate::cli::output::Event::Completed {
            summary: serde_json::json!({ "alias": alias, "deleted": true }),
        });
    })
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
    fn save_returns_powershell_error_when_cmdkey_unavailable() {
        let db = fresh_db();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let result = save(&mut ctx, "alias", "u", Some("p"), false, "winrm");
        // cmdkey is the first remote step; on non-Windows it fails as PowerShell.
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
        // SQLite must remain empty: no metadata written when cmdkey fails.
        assert_eq!(data_creds::list_all(&db).unwrap().len(), 0);
    }
}
