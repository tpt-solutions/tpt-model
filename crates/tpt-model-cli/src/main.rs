//! `tpt-model`: squash/compact tool.
//!
//! ```text
//! tpt-model squash --min-supported <N> [--patch <FILE>] [--apply] <PATH>...
//! ```
//!
//! Prints a plan. `--patch` writes a unified diff. `--apply` edits the files in place.
//! Exit codes: 0 success, 1 refused or failed, 2 usage error.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use tpt_model_cli::{analyze, apply, scan, SourceFile};

const USAGE: &str =
    "usage: tpt-model squash --min-supported <N> [--patch <FILE>] [--apply] <PATH>...";

#[derive(Debug, Default)]
struct Options {
    min_supported: Option<u32>,
    patch: Option<PathBuf>,
    apply: bool,
    paths: Vec<PathBuf>,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut it = args.iter();
    let mut subcommand_seen = false;
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "squash" if !subcommand_seen => subcommand_seen = true,
            "--min-supported" => {
                let v = it.next().ok_or("--min-supported needs a value")?;
                let n: u32 = v
                    .parse()
                    .map_err(|_| format!("--min-supported must be a number, got `{v}`"))?;
                if n == 0 {
                    return Err("--min-supported must be at least 1".into());
                }
                opts.min_supported = Some(n);
            }
            "--patch" => opts.patch = Some(PathBuf::from(it.next().ok_or("--patch needs a file")?)),
            "--apply" => opts.apply = true,
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with("--") => return Err(format!("unknown option `{other}`")),
            other => opts.paths.push(PathBuf::from(other)),
        }
    }
    if !subcommand_seen {
        return Err("expected the `squash` subcommand".into());
    }
    if opts.min_supported.is_none() {
        return Err("--min-supported is required".into());
    }
    if opts.paths.is_empty() {
        return Err("give at least one path to scan".into());
    }
    Ok(opts)
}

/// Collects `.rs` files under `path`, skipping `target` and hidden directories.
fn collect_rs(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_file() {
        if path.extension().is_some_and(|e| e == "rs") {
            out.push(path.to_path_buf());
        }
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(path)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for entry in entries {
        let name = entry.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if entry.is_dir() && (name == "target" || name.starts_with('.')) {
            continue;
        }
        collect_rs(&entry, out)?;
    }
    Ok(())
}

fn run(opts: &Options) -> Result<(), String> {
    let mut paths = Vec::new();
    for p in &opts.paths {
        collect_rs(p, &mut paths).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    if paths.is_empty() {
        return Err("no .rs files found under the given paths".into());
    }
    let files: Vec<SourceFile> = paths
        .iter()
        .map(|p| {
            fs::read_to_string(p)
                .map(|text| SourceFile {
                    path: p.clone(),
                    text,
                })
                .map_err(|e| format!("{}: {e}", p.display()))
        })
        .collect::<Result<_, _>>()?;

    let scanned = scan(&files)?;
    let min = opts.min_supported.unwrap_or(1);
    let plan = analyze(&scanned, min).map_err(|r| r.to_string())?;

    print!("{}", plan.render());

    if let Some(path) = &opts.patch {
        fs::write(path, plan.patch(&files)).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("patch written to {}", path.display());
    }
    if opts.apply {
        for updated in apply(&plan, &files) {
            if updated.text
                != files
                    .iter()
                    .find(|f| f.path == updated.path)
                    .map(|f| f.text.clone())
                    .unwrap_or_default()
            {
                fs::write(&updated.path, &updated.text)
                    .map_err(|e| format!("{}: {e}", updated.path.display()))?;
            }
        }
        println!(
            "applied {} removal(s); run `cargo check` to confirm",
            plan.removals.len()
        );
    } else if !plan.removals.is_empty() {
        println!("dry run: pass --apply to edit files");
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match parse(&args) {
        Ok(o) => o,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("error: {msg}");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
    }
}
