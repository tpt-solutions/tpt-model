//! `tpt-model`: squash/compact and inspection tool. See `tpt-model --help`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    tpt_model_cli::driver::run(&args)
}
