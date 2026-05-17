//! `uecm-cli share <action>` handlers.

use crate::cli::args::ShareAction;
use crate::cli::credential_args::CredentialArgs;
use crate::cli::output::{EmitSerialize, Event};
use crate::cli::run::Ctx;
use crate::data::share_configs::{self as data_shares, ShareMode};
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: ShareAction) -> UecmResult<()> {
    match action {
        ShareAction::List => list(ctx),
        ShareAction::Forget { id, yes } => forget(ctx, id, yes),
        ShareAction::Create { mode, host, share, local_path, cred } => {
            create(ctx, &mode, &host, &share, &local_path, &cred)
        }
        ShareAction::InjectSystemCred { client_host, target_host, svc_user, cred } => {
            inject_system_cred(ctx, &client_host, &target_host, &svc_user, &cred)
        }
    }
}

fn list(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = data_shares::list_all(db)?;
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}

fn forget(ctx: &mut Ctx<'_>, id: i64, yes: bool) -> UecmResult<()> {
    if !yes {
        return Err(UecmError::InvalidInput(
            "share forget is destructive; pass --yes to confirm. \
             Note: the remote SMB share is NOT removed by this command — \
             use ssh + Remove-SmbShare for that."
                .into(),
        ));
    }
    let db = ctx.require_db()?;
    data_shares::delete(db, id)?;
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "id": id,
                "forgotten": true,
                "note": "local inventory only; remote share still active",
            }),
        })
        .ok();
    Ok(())
}

fn create(
    ctx: &mut Ctx<'_>,
    mode: &str,
    host: &str,
    share: &str,
    local_path: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    use crate::core::shares;

    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let (op_user, op_pass) = match &creds {
        Some((u, p)) => (Some(u.as_str()), Some(p.as_str())),
        None => (None, None),
    };

    let (result, share_mode, credential_alias) = match mode {
        "a" | "A" => {
            let r = shares::create_mode_a(host, share, local_path, op_user, op_pass)?;
            (r, ShareMode::Open, cred.cred_alias.clone())
        }
        "b" | "B" => {
            let svc_user = "ddc-svc";
            let svc_pass = shares::generate_svc_password();
            let r = shares::create_mode_b(
                host,
                share,
                local_path,
                svc_user,
                &svc_pass,
                op_user,
                op_pass,
            )?;
            // Persist svc credential to cmdkey + DPAPI so subsequent
            // `inject-system-cred` / Robocopy fan-out can resolve it.
            let svc_alias = format!("share-{}-{}", host, share);
            crate::core::credentials::store(&svc_alias, svc_user, &svc_pass)?;
            if let Err(dpapi_err) =
                crate::core::credentials::store_password(&svc_alias, &svc_pass)
            {
                let _ = crate::core::credentials::delete(&svc_alias);
                return Err(dpapi_err);
            }
            (r, ShareMode::Managed, Some(svc_alias))
        }
        other => {
            return Err(UecmError::InvalidInput(format!(
                "unknown share mode '{}'; expected 'a' or 'b'",
                other
            )))
        }
    };

    // Resolve host's machine_id. Try by IP first; fall back to hostname match.
    let host_machine = crate::data::machines::find_by_ip(db, host)?
        .or_else(|| {
            crate::data::machines::list_all(db).ok().and_then(|rows| {
                rows.into_iter().find(|m| m.hostname == host)
            })
        })
        .ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "host '{}' is not in the machine inventory; run `machine add` first",
                host
            ))
        })?;
    let machine_id = host_machine.id.expect("machine from DB has id");

    let config = data_shares::ShareConfig {
        id: None,
        host_machine_id: machine_id,
        share_name: share.to_string(),
        unc_path: result.unc_path.clone(),
        local_path: local_path.to_string(),
        mode: share_mode,
        credential_alias,
    };
    let id = data_shares::insert(db, &config)?;

    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "id": id,
                "host": host,
                "share": share,
                "unc_path": result.unc_path,
                "mode": mode.to_uppercase(),
            }),
        })
        .ok();
    Ok(())
}

fn inject_system_cred(
    ctx: &mut Ctx<'_>,
    client_host: &str,
    target_host: &str,
    svc_user: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let (op_user, op_pass) = match &creds {
        Some((u, p)) => (Some(u.as_str()), Some(p.as_str())),
        None => (None, None),
    };

    // Look up the share's svc password from the DPAPI alias created during `share create`.
    // The alias scheme matches `share create`: `share-<host>-<share>`. For
    // inject-system-cred we only know target_host + svc_user, so we look for
    // ANY alias starting with `share-<target_host>-`.
    let svc_pass = find_share_svc_password(db, target_host, svc_user)?;

    let message = crate::core::psexec::inject_system_credential(
        client_host,
        target_host,
        svc_user,
        &svc_pass,
        op_user,
        op_pass,
    )?;

    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "client_host": client_host,
                "target_host": target_host,
                "svc_user": svc_user,
                "message": message,
            }),
        })
        .ok();
    Ok(())
}

/// Looks up the svc password for any Mode-B share on `target_host`. Returns
/// `InvalidInput` if no matching alias exists in DPAPI / SQLite.
fn find_share_svc_password(
    db: &crate::data::Db,
    target_host: &str,
    _svc_user: &str,
) -> UecmResult<String> {
    let prefix = format!("share-{}-", target_host);
    let candidates: Vec<_> = data_shares::list_all(db)?
        .into_iter()
        .filter_map(|s| s.credential_alias)
        .filter(|a| a.starts_with(&prefix))
        .collect();
    let alias = candidates.into_iter().next().ok_or_else(|| {
        UecmError::InvalidInput(format!(
            "no Mode B share found for host '{}'; create one via `share create --mode b` first",
            target_host
        ))
    })?;
    crate::core::credentials::resolve_password(&alias).map_err(|e| {
        UecmError::InvalidInput(format!(
            "DPAPI lookup for alias '{}' failed: {}. The share may have been created \
             outside this CLI; re-create or recover the svc password manually.",
            alias, e
        ))
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

    #[test]
    fn forget_without_yes_returns_invalid_input() {
        let db = fresh_db();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        assert!(matches!(forget(&mut ctx, 1, false), Err(UecmError::InvalidInput(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn create_unknown_mode_returns_invalid_input() {
        let db = fresh_db();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let cred = CredentialArgs { cred_alias: None, user: None, pass: None, pass_stdin: false };
        let r = create(&mut ctx, "z", "host", "share", "C:\\path", &cred);
        assert!(matches!(r, Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn list_empty_db_returns_empty_vec() {
        let db = fresh_db();
        let mut buf: Vec<u8> = Vec::new();
        let r = {
            let mut ctx = make_ctx(&mut buf, &db);
            list(&mut ctx)
        };
        assert!(r.is_ok());
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("[]"));
    }

    #[test]
    fn inject_system_cred_no_share_returns_invalid_input() {
        let db = fresh_db();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let cred = CredentialArgs { cred_alias: None, user: None, pass: None, pass_stdin: false };
        let r = inject_system_cred(&mut ctx, "client", "192.0.2.2", "ddc-svc", &cred);
        assert!(matches!(r, Err(UecmError::InvalidInput(_))));
    }
}
