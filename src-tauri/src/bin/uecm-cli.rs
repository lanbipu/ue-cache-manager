//! `uecm-cli` entry point. Parses args, hands off to `uecm_lib::cli::run`.

use clap::Parser;
use uecm_lib::cli::args::Cli;
use uecm_lib::cli::run;

fn main() {
    let cli = Cli::parse();
    let code = run::run(cli);
    std::process::exit(code);
}
