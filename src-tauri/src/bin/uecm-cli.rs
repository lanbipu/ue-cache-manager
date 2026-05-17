//! `uecm-cli` entry point.
//!
//! Two-stage parse:
//! 1. Sniff `--json` from raw argv so we know how to format clap parse errors
//!    (the structured `Cli` is not available yet at that point).
//! 2. Try the real parse; on failure emit a JSON error envelope to stderr
//!    (exit 64 = sysexits.h EX_USAGE) when `--json` is set, otherwise let
//!    clap render its native usage message and exit with its default code.

use clap::error::ErrorKind;
use clap::Parser;
use std::ffi::OsString;
use std::io::{self, Write};
use uecm_lib::cli::args::Cli;
use uecm_lib::cli::run;

fn main() {
    // Use args_os to tolerate non-UTF-8 paths (e.g. someone passes a binary
    // --db-path on Unix). `args()` would panic before clap can parse.
    let argv: Vec<OsString> = std::env::args_os().collect();
    let json_mode = argv.iter().any(|a| a.as_os_str() == "--json");

    match Cli::try_parse_from(&argv) {
        Ok(cli) => {
            let code = run::run(cli);
            std::process::exit(code);
        }
        Err(e) => {
            // `--help` and `--version` are clap "errors" that print to stdout
            // and exit 0. Pass those through unchanged. Missing-subcommand /
            // missing-required-arg are real usage errors and go through the
            // exit-64 path below so automation can distinguish argv-shape
            // failures (64) from handler-level invalid_input (2).
            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
                e.exit();
            }

            if json_mode {
                let payload = serde_json::json!({
                    "kind": "error",
                    "code": "usage_error",
                    "message": e.to_string(),
                    "clap_kind": format!("{:?}", e.kind()),
                });
                let mut stderr = io::stderr().lock();
                let _ = serde_json::to_writer(&mut stderr, &payload);
                let _ = stderr.write_all(b"\n");
                std::process::exit(64);
            } else {
                // Reproduce clap's native rendering on stderr, then exit 64
                // so non-JSON automation can still distinguish usage errors
                // from runtime failures.
                let _ = writeln!(io::stderr(), "{}", e);
                std::process::exit(64);
            }
        }
    }
}
