//! `uecm-cli system <action>` handlers.

use crate::cli::args::SystemAction;
use crate::cli::output::Event;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;   // brings emit_result(&T) into scope on dyn Emitter
use crate::error::UecmResult;
use crate::startup;
use serde::Serialize;

#[derive(Serialize)]
struct VersionInfo {
    binary: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct PathInfo {
    path: String,
}

pub fn handle(ctx: &mut Ctx<'_>, action: SystemAction) -> UecmResult<()> {
    match action {
        SystemAction::Version => version(ctx),
        SystemAction::DbPath => db_path(ctx),
        SystemAction::PsDir => ps_dir(ctx),
        SystemAction::MigrateDb => migrate_db(ctx),
        SystemAction::Echo { message } => echo(ctx, &message),
    }
}

fn version(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let info = VersionInfo { binary: "uecm-cli", version: env!("CARGO_PKG_VERSION") };
    ctx.emitter.emit_result(&info).ok();
    Ok(())
}

fn db_path(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    // Report the path the CLI actually opened, respecting `--db-path` / `UECM_DB_PATH`.
    let info = PathInfo { path: ctx.db_path.to_string_lossy().into() };
    ctx.emitter.emit_result(&info).ok();
    Ok(())
}

fn ps_dir(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let path = startup::resolve_ps_script_dir();
    let info = PathInfo { path: path.to_string_lossy().into() };
    ctx.emitter.emit_result(&info).ok();
    Ok(())
}

fn migrate_db(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    // Re-runs migration on the SAME DB the CLI opened (no path re-resolution).
    // open_and_migrate_db is idempotent so running here is a no-op if startup already ran.
    let _ = startup::open_and_migrate_db(&ctx.db_path)?;
    let summary = serde_json::json!({ "migrated": true, "path": ctx.db_path.to_string_lossy() });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}

fn echo(ctx: &mut Ctx<'_>, message: &str) -> UecmResult<()> {
    let result: serde_json::Value = crate::core::powershell::run_json(
        &crate::core::powershell::script_path("test-echo.ps1"),
        &["-Message", message],
    )?;
    ctx.emitter.emit_value(&result).ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::output::{Emitter, NdjsonEmitter};
    use crate::data::open_in_memory;

    #[test]
    fn test_version_info() {
        // Sanity check: VersionInfo should serialize with correct fields.
        let info = VersionInfo { binary: "uecm-cli", version: "0.1.0" };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["binary"], "uecm-cli");
        assert_eq!(json["version"], "0.1.0");
    }

    #[test]
    fn test_path_info() {
        // Sanity check: PathInfo should serialize correctly.
        let info = PathInfo { path: "/some/path".to_string() };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["path"], "/some/path");
    }

    #[cfg(not(windows))]
    #[test]
    fn echo_returns_powershell_error_on_non_windows() {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            crate::data::schema::migrate(&mut conn).unwrap();
        }
        let mut buf: Vec<u8> = Vec::new();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(&mut buf));
        let mut ctx = Ctx {
            db: Some(db),
            db_path: std::path::PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        };
        let result = echo(&mut ctx, "hello");
        assert!(matches!(result, Err(crate::error::UecmError::PowerShell(_))));
    }
}
