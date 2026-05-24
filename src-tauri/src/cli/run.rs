//! Top-level dispatch. Bin entry parses args, builds emitter, opens DB only
//! when the requested command needs it, hands off to domain.

use crate::cli::args::{Cli, Domain};
use crate::cli::output::{Emitter, HumanEmitter, NdjsonEmitter, exit_code_for};
use crate::cli::{domain_cred, domain_ddc, domain_deploy, domain_env, domain_gpu, domain_health, domain_ini, domain_local_cache, domain_machine, domain_project, domain_pso, domain_secret, domain_share, domain_ssh, domain_system, domain_winrm, domain_zen};
use crate::data::Db;
use crate::error::UecmError;
use crate::startup;
use std::io::{self, Write};
use std::path::PathBuf;

pub struct Ctx<'a> {
    /// `None` for diagnostic / write-free commands (e.g. `system version`,
    /// `winrm bootstrap-script`). Handlers that need DB access must call
    /// `ctx.require_db()` and propagate the error.
    pub db: Option<Db>,
    /// The DB path the CLI would open / opened. Handlers MUST use this rather
    /// than re-resolving via `startup::resolve_db_path()`, otherwise CLI-level
    /// `--db-path` overrides become inconsistent between commands.
    pub db_path: PathBuf,
    pub emitter: Box<dyn Emitter + 'a>,
    pub json_mode: bool,
}

impl<'a> Ctx<'a> {
    /// Convenience for DB-requiring handlers. Panics with a structured
    /// `UecmError` if `needs_db` was wrong — never panics in correct code.
    pub fn require_db(&self) -> Result<&Db, UecmError> {
        self.db.as_ref().ok_or_else(|| {
            UecmError::OperationFailed(
                "internal: this command requires a DB but Ctx was built DB-less".into(),
            )
        })
    }
}

/// Whether a parsed command actually needs SQLite to be open.
///
/// DB-free commands (system version / db-path / ps-dir, winrm bootstrap-script)
/// remain runnable even when the data directory is unwritable or the DB file
/// is broken. Per Codex review feedback on Task 1.4 / 2.1.
fn needs_db(cmd: &Domain) -> bool {
    use crate::cli::args::{MachineAction, SystemAction, ZenAction};
    match cmd {
        // `machine scan` is a stateless network probe — no DB writes.
        // Everything else in the Machine domain reads or writes tables.
        Domain::Machine { action } => !matches!(action, MachineAction::Scan { .. }),
        // `system echo` and the path-printing variants don't open DB;
        // only `migrate-db` does.
        Domain::System { action } => matches!(action, SystemAction::MigrateDb),
        // None of the WinRM commands touch SQLite — they all talk PowerShell
        // sidecar or print text.
        Domain::Winrm { .. } => false,
        // `ssh probe` spawns ssh; `ssh package-bootstrap` touches the keystore +
        // file packager. Neither needs SQLite.
        Domain::Ssh { .. } => false,
        // `secret` talks only to the file-backed SecretStore; no DB.
        Domain::Secret { .. } => false,
        // Phase 2 domains (cred/env/ini/share) — all will require DB for now.
        Domain::Cred { .. } => true,
        Domain::Env { .. } => true,
        Domain::Ini { .. } => true,
        Domain::Share { .. } => true,
        Domain::Project { .. } => true,
        Domain::Health { .. } => true,
        Domain::Gpu { .. } => true,
        Domain::Ddc { .. } => true,
        Domain::Pso { .. } => true,
        Domain::Log { .. } => true,
        Domain::LocalCache { .. } => true,
        Domain::Deploy { .. } => true,
        // Most zen commands need DB (endpoints / probes / cache_stats /
        // baselines / binary inventory rows). Exception: `zen verify-rules`
        // is a pure yaml-resolver in the resolve-only mode — no DB access.
        // Codex P2 fix from T4.5 review: forcing DB-open here makes the
        // command unusable when the data dir is read-only or the SQLite
        // file is broken, even though verify-rules only needs the embedded
        // yaml.
        //
        // T4.4 added `--run-editor` which DOES need DB (machine lookup +
        // operations row). Re-enable DB-open for that branch only.
        Domain::Zen { action } => match action {
            ZenAction::VerifyRules { run_editor, .. } => *run_editor,
            _ => true,
        },
    }
}

pub fn run(cli: Cli) -> i32 {
    // tracing init
    let filter = tracing_subscriber::EnvFilter::try_new(&cli.log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .try_init();

    // DB path resolves cheaply (no I/O beyond create_dir_all on default path).
    // Doing it unconditionally keeps `system db-path` working without DB.
    let db_path = match cli.db_path.clone() {
        Some(p) => PathBuf::from(p),
        None => match startup::resolve_db_path() {
            Ok(p) => p,
            Err(e) => return finish_error(&e, cli.json),
        },
    };

    // Only open + migrate the DB if the chosen command actually uses it.
    let db = if needs_db(&cli.command) {
        match startup::open_and_migrate_db(&db_path) {
            Ok(db) => Some(db),
            Err(e) => return finish_error(&e, cli.json),
        }
    } else {
        None
    };

    // Emitter
    let json_mode = cli.json;
    let stdout = io::stdout();
    let stderr = io::stderr();
    let emitter: Box<dyn Emitter> = if json_mode {
        Box::new(NdjsonEmitter::new(stdout.lock()))
    } else {
        let color = atty::is(atty::Stream::Stdout);
        Box::new(HumanEmitter::new(stdout.lock(), stderr.lock(), color))
    };

    let mut ctx = Ctx { db, db_path, emitter, json_mode };

    let result = match cli.command {
        Domain::System { action } => domain_system::handle(&mut ctx, action),
        Domain::Machine { action } => domain_machine::handle(&mut ctx, action),
        Domain::Winrm { action } => domain_winrm::handle(&mut ctx, action),
        Domain::Ssh { action } => domain_ssh::handle(&mut ctx, action),
        Domain::Cred { action } => domain_cred::handle(&mut ctx, action),
        Domain::Secret { action } => domain_secret::handle(&mut ctx, action),
        Domain::Env { action } => domain_env::handle(&mut ctx, action),
        Domain::Ini { action } => domain_ini::handle(&mut ctx, action),
        Domain::Share { action } => domain_share::handle(&mut ctx, action),
        Domain::Project { action } => domain_project::handle(&mut ctx, action),
        Domain::Health { action } => domain_health::handle(&mut ctx, action),
        Domain::Gpu { action } => domain_gpu::handle(&mut ctx, action),
        Domain::Ddc { action } => domain_ddc::handle(&mut ctx, action),
        Domain::Pso { action } => domain_pso::handle(&mut ctx, action),
        Domain::Log { action } => crate::cli::domain_log::handle(&mut ctx, action),
        Domain::LocalCache { action } => domain_local_cache::handle(&mut ctx, action),
        Domain::Deploy { action } => domain_deploy::handle(&mut ctx, action),
        Domain::Zen { action } => domain_zen::handle(&mut ctx, action),
    };

    match result {
        Ok(()) => 0,
        Err(e) => {
            ctx.emitter.emit_error(&e);
            exit_code_for(&e)
        }
    }
}

fn finish_error(err: &UecmError, json: bool) -> i32 {
    if json {
        // `NdjsonEmitter::new` routes errors to stderr per spec §4.
        let mut e = NdjsonEmitter::new(io::stdout().lock());
        e.emit_error(err);
    } else {
        let _ = writeln!(io::stderr(), "✗ {}", err);
    }
    exit_code_for(err)
}
