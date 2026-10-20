# Changelog

All notable changes to `tpt-model-cli` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace-wide log is in the
[root changelog](../../CHANGELOG.md).

## [Unreleased]

### Changed

* The emitted patch has three lines of context, so `git apply` accepts it.
* `--apply` restores files if a write fails. Malformed `#[tpt]` attributes are errors.
* Keywords and categories are set per crate, and the crate has its own README.

### Added

* `tpt-model squash`: plans removal of versions below `--min-supported`, refuses when kept code still
  uses a removed type, and writes a patch (`--patch`), applies it (`--apply`), or prints JSON (`--json`).
* `tpt-model inspect FILE`, with `--key-hex` for keyed data.
* `tpt-model new-migration --name TYPE PATH...`: prints the next version and a migration stub.
* `cargo tpt-model` as a second binary, sharing the driver with `tpt-model`.
* Full `--help` output.

### Fixed

* `squash` refuses when a kept item shares a line with a removed item (`SharesLines`). Removal works
  on whole lines, so before this the kept item would have been deleted too.
* CRLF line endings are preserved on `--apply`.
* Rustdoc: the CLI binaries no longer clash with the library's documentation output.

## [0.1.0]

Not released yet. The first release will be tagged after the v1.0.0 decision.
