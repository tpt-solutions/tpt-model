//! `cargo tpt-model`: the same tool, run as a cargo subcommand.
//!
//! Cargo passes the subcommand name as the first argument, so it is dropped here.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("tpt-model") {
        args.remove(0);
    }
    tpt_model_cli::driver::run(&args)
}
