//! `uecm-cli secret <action>` — manage the cross-platform SecretStore (AES-GCM)
//! directly. Lets operators / agents store and inspect transport secrets (Mode B
//! share svc passwords, saved WinRM aliases) from the command line. Replaces the
//! retiring `cred` domain's DPAPI write path with the SSH-era SecretStore.

use std::io::{self, BufRead};

use crate::cli::args::SecretAction;
use crate::cli::destructive::{self, Outcome};
use crate::cli::output::EmitSerialize;
use crate::cli::run::Ctx;
use crate::core::secrets::SecretStore;
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: SecretAction) -> UecmResult<()> {
    match action {
        SecretAction::Set { alias, value } => set(ctx, &alias, value),
        SecretAction::Get { alias } => get(ctx, &alias),
        SecretAction::List => list(ctx),
        SecretAction::Delete { alias, yes, dry_run } => delete(ctx, &alias, yes, dry_run),
    }
}

fn set(ctx: &mut Ctx<'_>, alias: &str, value: Option<String>) -> UecmResult<()> {
    let secret = match value {
        Some(v) => v,
        None => {
            // No --value: read one line from stdin (mirrors --pass-stdin), so the
            // secret never lands in shell history.
            let mut line = String::new();
            io::stdin()
                .lock()
                .read_line(&mut line)
                .map_err(|e| UecmError::InvalidInput(format!("read secret from stdin: {}", e)))?;
            line.trim_end_matches(['\r', '\n']).to_string()
        }
    };
    if secret.is_empty() {
        return Err(UecmError::InvalidInput(
            "secret value is empty (pass --value or pipe it on stdin)".into(),
        ));
    }
    SecretStore::from_config()?.put(alias, &secret)?;
    // Never echo the secret — report only the alias + length.
    ctx.emitter
        .emit_result(&serde_json::json!({
            "alias": alias,
            "stored": true,
            "value_len": secret.chars().count(),
        }))
        .ok();
    Ok(())
}

fn get(ctx: &mut Ctx<'_>, alias: &str) -> UecmResult<()> {
    let value = SecretStore::from_config()?.get(alias)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("no secret stored for alias '{}'", alias))
    })?;
    // `secret get` is the one place the plaintext intentionally surfaces.
    ctx.emitter
        .emit_result(&serde_json::json!({ "alias": alias, "value": value }))
        .ok();
    Ok(())
}

fn list(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let aliases = SecretStore::from_config()?.list()?;
    ctx.emitter
        .emit_result(&serde_json::json!({ "aliases": aliases }))
        .ok();
    Ok(())
}

fn delete(ctx: &mut Ctx<'_>, alias: &str, yes: bool, dry_run: bool) -> UecmResult<()> {
    let outcome = destructive::check(yes, dry_run, "secret.delete")?;
    if outcome == Outcome::DryRun {
        let exists = SecretStore::from_config()?.get(alias)?.is_some();
        destructive::emit_plan(
            ctx.emitter.as_mut(),
            "secret.delete",
            serde_json::json!({ "alias": alias, "exists": exists }),
        );
        return Ok(());
    }
    SecretStore::from_config()?.delete(alias)?;
    ctx.emitter
        .emit_result(&serde_json::json!({ "alias": alias, "deleted": true }))
        .ok();
    Ok(())
}
