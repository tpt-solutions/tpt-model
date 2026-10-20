//! Command-line driver, shared by the `tpt-model` and `cargo-tpt-model` binaries.

use crate::{analyze, apply, inspect, scaffold_migration, scan, SourceFile};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub const HELP: &str = "\
tpt-model: schema evolution tools for tpt-model projects

USAGE:
    tpt-model squash --min-supported <N> [--json] [--patch <FILE>] [--apply] <PATH>...
    tpt-model inspect [--key-hex <HEX>] <FILE>
    tpt-model new-migration --name <TYPE> <PATH>...

COMMANDS:
    squash         Plan the removal of versions below N in the .rs files under PATH. Prints
                   the plan. --patch writes it as a unified diff; --apply edits the files and
                   restores them if a write fails.
    inspect        Print the envelope header and the stored tree of one sealed entity. The
                   hash is checked with unkeyed BLAKE3, or with --key-hex when keyed.
    new-migration  Print Rust source for the next version of TYPE: a copy of its latest
                   version, bumped, with a migration stub from the latest version. Paste it
                   into the source; nothing is written.

OPTIONS:
    --min-supported <N>  Oldest version to keep, at least 1 (required by squash)
    --name <TYPE>        Logical type name (required by new-migration)
    --json               Print the squash plan as JSON
    --patch <FILE>       Write the squash plan as a unified diff
    --apply              Edit the files in place
    --key-hex <HEX>      64 hex digits: a 32-byte key for a keyed digest
    -h, --help           Print this help

EXIT CODES:
    0 success, 1 refused or failed, 2 usage error
";

const USAGE: &str =
    "usage: tpt-model squash --min-supported <N> [--json] [--patch <FILE>] [--apply] <PATH>...
       tpt-model inspect [--key-hex <HEX>] <FILE>
Run `tpt-model --help` for details.";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SquashOptions {
    pub min_supported: Option<u32>,
    pub patch: Option<PathBuf>,
    pub apply: bool,
    pub json: bool,
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Squash(SquashOptions),
    Inspect {
        key: Option<[u8; 32]>,
        file: PathBuf,
    },
    NewMigration {
        name: String,
        paths: Vec<PathBuf>,
    },
}

/// Parses the arguments after the program name. A usage problem is `Err`.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let mut it = args.iter();
    let Some(first) = it.next() else {
        return Err("expected a command: `squash` or `inspect`".into());
    };
    match first.as_str() {
        "-h" | "--help" => Ok(Command::Help),
        "squash" => parse_squash(it),
        "inspect" => parse_inspect(it),
        "new-migration" => parse_new_migration(it),
        other => Err(format!("unknown command `{other}`")),
    }
}

fn parse_new_migration<'a>(mut it: impl Iterator<Item = &'a String>) -> Result<Command, String> {
    let mut name = None;
    let mut paths = Vec::new();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--name" => name = Some(it.next().ok_or("--name needs a value")?.clone()),
            "-h" | "--help" => return Ok(Command::Help),
            other if other.starts_with("--") => return Err(format!("unknown option `{other}`")),
            other => paths.push(PathBuf::from(other)),
        }
    }
    let name = name.ok_or("new-migration needs --name <TYPE>")?;
    if paths.is_empty() {
        return Err("give at least one path to scan".into());
    }
    Ok(Command::NewMigration { name, paths })
}

fn parse_squash<'a>(mut it: impl Iterator<Item = &'a String>) -> Result<Command, String> {
    let mut opts = SquashOptions::default();
    while let Some(arg) = it.next() {
        match arg.as_str() {
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
            "--json" => opts.json = true,
            "-h" | "--help" => return Ok(Command::Help),
            other if other.starts_with("--") => return Err(format!("unknown option `{other}`")),
            other => opts.paths.push(PathBuf::from(other)),
        }
    }
    if opts.min_supported.is_none() {
        return Err("--min-supported is required".into());
    }
    if opts.paths.is_empty() {
        return Err("give at least one path to scan".into());
    }
    Ok(Command::Squash(opts))
}

fn parse_inspect<'a>(mut it: impl Iterator<Item = &'a String>) -> Result<Command, String> {
    let mut key = None;
    let mut file = None;
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--key-hex" => key = Some(parse_key(it.next().ok_or("--key-hex needs a value")?)?),
            "-h" | "--help" => return Ok(Command::Help),
            other if other.starts_with("--") => return Err(format!("unknown option `{other}`")),
            other if file.is_none() => file = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument `{other}`")),
        }
    }
    let file = file.ok_or("inspect needs a file")?;
    Ok(Command::Inspect { key, file })
}

/// Parses 64 hex digits into a 32-byte key.
fn parse_key(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("--key-hex needs exactly 64 hex digits".into());
    }
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(key)
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

/// Writes the changed files. If a write fails, the files already written are restored
/// from `originals`, so a failed `--apply` leaves the tree as it was.
fn write_all(updated: &[SourceFile], originals: &[SourceFile]) -> Result<(), String> {
    let mut written: Vec<&SourceFile> = Vec::new();
    for file in updated {
        let Some(original) = originals.iter().find(|f| f.path == file.path) else {
            continue;
        };
        if file.text == original.text {
            continue;
        }
        if let Err(e) = fs::write(&file.path, &file.text) {
            let mut message = format!("{}: {e}", file.path.display());
            for done in written {
                if fs::write(&done.path, &done.text).is_err() {
                    message.push_str(&format!("; could not restore {}", done.path.display()));
                }
            }
            return Err(message);
        }
        written.push(original);
    }
    Ok(())
}

/// Reads every `.rs` file under `paths`.
fn load_sources(paths: &[PathBuf]) -> Result<Vec<SourceFile>, String> {
    let mut found = Vec::new();
    for p in paths {
        collect_rs(p, &mut found).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    if found.is_empty() {
        return Err("no .rs files found under the given paths".into());
    }
    found
        .into_iter()
        .map(|p| {
            fs::read_to_string(&p)
                .map(|text| SourceFile {
                    path: p.clone(),
                    text,
                })
                .map_err(|e| format!("{}: {e}", p.display()))
        })
        .collect()
}

fn squash(opts: &SquashOptions) -> Result<(), String> {
    let files = load_sources(&opts.paths)?;
    let scanned = scan(&files)?;
    let min = opts.min_supported.unwrap_or(1);
    let plan = analyze(&scanned, min).map_err(|r| r.to_string())?;

    if opts.json {
        print!("{}", plan.to_json());
    } else {
        print!("{}", plan.render());
    }

    if let Some(path) = &opts.patch {
        fs::write(path, plan.patch(&files)).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("patch written to {}", path.display());
    }
    if opts.apply {
        write_all(&apply(&plan, &files), &files)?;
        println!(
            "applied {} removal(s); run `cargo check` to confirm",
            plan.removals.len()
        );
    } else if !plan.removals.is_empty() && !opts.json {
        println!("dry run: pass --apply to edit files");
    }
    Ok(())
}

fn inspect_file(file: &Path, key: Option<[u8; 32]>) -> Result<(), String> {
    let bytes = fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    print!("{}", inspect::render(&bytes, key)?);
    Ok(())
}

/// Runs the command line and returns the process exit code.
pub fn run(args: &[String]) -> ExitCode {
    let command = match parse(args) {
        Ok(command) => command,
        Err(msg) => {
            eprintln!("error: {msg}");
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = match command {
        Command::Help => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Command::Squash(opts) => squash(&opts),
        Command::Inspect { key, file } => inspect_file(&file, key),
        Command::NewMigration { name, paths } => load_sources(&paths).and_then(|files| {
            print!("{}", scaffold_migration(&files, &name)?);
            Ok(())
        }),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
    }
}
