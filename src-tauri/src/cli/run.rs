//! Top-level dispatch. Bin entry parses args, builds emitter, opens DB, hands off to domain.

use crate::cli::args::{Cli, Domain};
use crate::cli::output::{Emitter, HumanEmitter, NdjsonEmitter, exit_code_for};
use crate::cli::{domain_machine, domain_system, domain_winrm};
use crate::data::Db;
use crate::error::UecmError;
use crate::startup;
use std::io::{self, Write};
use std::path::PathBuf;

pub struct Ctx<'a> {
    pub db: &'a Db,
    /// The DB path actually opened by `run()`. Handlers MUST use this rather
    /// than re-resolving via `startup::resolve_db_path()`, otherwise CLI-level
    /// `--db-path` overrides become inconsistent between commands.
    pub db_path: PathBuf,
    pub emitter: Box<dyn Emitter + 'a>,
    pub json_mode: bool,
}

pub fn run(cli: Cli) -> i32 {
    // tracing init
    let filter = tracing_subscriber::EnvFilter::try_new(&cli.log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .try_init();

    // DB
    let db_path = match cli.db_path.clone() {
        Some(p) => PathBuf::from(p),
        None => match startup::resolve_db_path() {
            Ok(p) => p,
            Err(e) => return finish_error(&e, cli.json),
        },
    };
    let db = match startup::open_and_migrate_db(&db_path) {
        Ok(db) => db,
        Err(e) => return finish_error(&e, cli.json),
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

    let mut ctx = Ctx { db: &db, db_path: db_path.clone(), emitter, json_mode };

    let result = match cli.command {
        Domain::System { action } => domain_system::handle(&mut ctx, action),
        Domain::Machine { action } => domain_machine::handle(&mut ctx, action),
        Domain::Winrm { action } => domain_winrm::handle(&mut ctx, action),
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
        let mut e = NdjsonEmitter::new(io::stdout().lock());
        e.emit_error(err);
    } else {
        let _ = writeln!(io::stderr(), "✗ {}", err);
    }
    exit_code_for(err)
}
