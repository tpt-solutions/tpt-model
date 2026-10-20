# tpt-model-cli

Command-line tool for [`tpt-model`](../tpt-model) projects. It works on the source code, not on
compiled types, and it never changes stored data.

* `squash`: finds versioned types and migrations below a minimum supported version. It writes a
  plan, a unified-diff patch, or applies the removal.
* `inspect`: shows the envelope header, schema id, version and tree of a stored entity.
* `new-migration`: scaffolds the next version of a type and a migration stub.

The crate also builds a `cargo tpt-model` subcommand (`cargo-tpt-model`), which runs the same driver.

This crate is not published to crates.io (`publish = false`). Build it from the workspace.

## Installation

From a checkout of the workspace:

```sh
cargo install --path crates/tpt-model-cli
```

or run it without installing:

```sh
cargo run -p tpt-model-cli -- --help
```

## Commands

### `squash`: remove old versions

```sh
tpt-model squash --min-supported 2 src/              # print the plan
tpt-model squash --min-supported 2 --patch squash.diff src/   # write a unified diff
tpt-model squash --min-supported 2 --apply src/      # remove the items in place
tpt-model squash --min-supported 2 --json src/       # plan as JSON, for tooling
```

* `--min-supported <N>`: the lowest version you still support. Versions below it, and migrations
  from them, are candidates for removal.
* `--patch <FILE>`: write the plan as a unified diff. It has three lines of context and applies
  with `git apply`.
* `--apply`: remove the items. Files are rewritten with their original line endings (CRLF is
  preserved). If a later write fails, files already written are restored.
* `--json`: the plan as JSON.

The tool **refuses** when code that stays still refers to something it would delete, and it
reports the reason and location. Matching is by name, not by module path. A name clash can cause a
refusal, which is the safe direction.

Only top-level items, including those in inline modules, are removed. Items are removed by line
span, so their attributes and doc comments go too. An item that shares a line with a removed item
is a refusal, because the whole line would be lost.

The tool cannot see persisted data. Before removing a version, confirm no stored data still needs it.

### `inspect`: look inside a stored entity

```sh
tpt-model inspect path/to/entity.bin
tpt-model inspect path/to/entity.bin --key-hex <64 hex digits>   # for data written with a keyed digest
```

### `new-migration`: scaffold the next version

```sh
tpt-model new-migration --name CadPart src/part.rs
```

Prints Rust source to stdout: the next version of `CadPart` (a copy of the latest, with the
version bumped) and a migration stub. Nothing is written to disk; copy the text into your module.

## Output and errors

`--help` prints the full usage. Malformed `#[tpt]` and `#[derive]` attributes are reported as errors.

## Library use

The analysis is also a library, for tests or CI: `tpt_model_cli::analyze`, `apply`, `scan` and
`scaffold_migration`, plus the `driver` module that the binary uses.

## Minimum supported Rust version

1.90, checked in CI.

## Licence

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
