//! `uecm-cli ini <action>` handlers.

use crate::cli::args::IniAction;
use crate::cli::credential_args::CredentialArgs;
use crate::cli::host_args::HostTarget;
use crate::cli::output::{EmitSerialize, Event};
use crate::cli::run::Ctx;
use crate::core::ini_editor;
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
    if value.len() >= 4 && msg.contains(value) {
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
        IniAction::Set { target, file, section, key, value, cred } => {
            let t = target.require_one()?;
            match t {
                HostTarget::Single(h) => set_single(ctx, &h, &file, &section, &key, &value, &cred),
                HostTarget::Batch(hs) => set_batch(ctx, &hs, &file, &section, &key, &value, &cred),
            }
        }
        IniAction::Remove { target, file, section, key, cred } => {
            let t = target.require_one()?;
            match t {
                HostTarget::Single(h) => remove_single(ctx, &h, &file, &section, &key, &cred),
                HostTarget::Batch(hs) => remove_batch(ctx, &hs, &file, &section, &key, &cred),
            }
        }
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
