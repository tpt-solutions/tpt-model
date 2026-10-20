# Changelog

All notable changes to `tpt-model-derive` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace-wide log is in the
[root changelog](../../CHANGELOG.md).

## [Unreleased]

### Changed

* Keywords and categories are set per crate, and the crate has its own README.

### Added

* `#[derive(Model)]` for structs and enums, with `version`, `name`, `since`, `removed_in`,
  `migrate`, `default`, `rename`, `intent`, `capability`, `min` and `max`.
* Compile-time checks for version ranges, removed fields without `migrate`, unknown attributes,
  and migration function signatures.
* Variant attributes `since`, `removed_in` and `migrate` for enums.
* Compile errors for `u128`, `i128`, arrays and tuples.
* `trybuild` compile-fail tests in `tests/ui/`.

### Fixed

* The derive reports 128-bit integers, arrays and tuples as compile errors instead of
  generating code that does not compile.

## [0.1.0]

Not released yet. The first release will be tagged after the v1.0.0 decision.
