# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed (breaking, before 1.0)

* `access::redact` takes a `&Registry` and redacts nested structs, enum payloads and sequences.
  Nested types the registry cannot resolve are replaced by `null`.
* `load` checks the stored tree against its source-version schema before migrating. Failures
  are reported as `LoadError::SourceShape`, which replaces the earlier `MigrationError::InvalidResult`
  for data that was already invalid at its own version.
* `EntityId` deserialization accepts only UUIDv7 and works from self-describing formats.
  Use `EntityId::try_from_bytes` for untrusted bytes.
* `tpt-model-cli`: the emitted patch has three lines of context, so `git apply` accepts it.
  `--apply` restores files if a write fails. Malformed `#[tpt]` attributes are errors.

### Added (second pass)

* `KeyedBlake3` in `tpt-crypto-stub`: keyed digest that rejects forgeries. Example `keyed_digest`.
* `export::typescript`: TypeScript declarations for a registry, using each type's latest version.
* `tpt-model inspect FILE` (with `--key-hex`), `squash --json`, and full `--help`.
* `cargo tpt-model` as a second binary, sharing the driver with `tpt-model`.
* Examples `quickstart` (the README code) and `squash_demo/` source for the CLI.
* `CONTRIBUTING.md` and `SECURITY.md`. Dependabot configuration.

### Added (fourth pass)

* `assert_schema_snapshot!` and `snapshot::check`: schema snapshot tests with `TPT_UPDATE_SNAPSHOTS`.
* Generated proptests across versions (`tests/evolution_proptest.rs`) and a derive shape snapshot.
* `templates/model-crate/`: a `cargo-generate` template for a new model crate.
* CI: 32-bit Windows job, coverage job (report only), MSRV job, changelog check.

### Added (third pass)

* `check::breaking_changes` and `check::report`: a breaking-change linter over two registries.
* `evolve::explain`: the plan for a pair of versions, field by field, without running it.
* `export::json_schema`: JSON Schema (draft 2020-12) for a registry.
* `assert_evolves!`: a macro for downstream tests that one saved value loads as another.
* `tpt-model-cli new-migration --name TYPE PATH...`: prints the next version and a migration stub.
* Examples `config_file`, `save_game`, `rename_and_split`, `custom_migration`.
* `docs/TUTORIAL.md`, `docs/COOKBOOK.md`, `docs/FAQ.md`.
* Fuzz target `envelope_valid`, which seals fuzz input in a correctly hashed envelope.

### Changed


* `rust-version` is 1.90. The dev-dependencies and `uuid` need newer compilers than 1.85.
* `deny.toml` `[advisories]` has an explicit ignore list. This cargo-deny version has no severity keys.

### Fixed
* `squash` refuses when a kept item shares a line with a removed item (`SharesLines`). Removal works on whole lines, so before this the kept item would have been deleted too.

* `SerdeError` implements `core::error::Error`, so the `no_std` crate builds and tests with
  `--no-default-features` alongside dev-dependencies that enable serde's `std`.
* Rustdoc: the CLI binaries no longer clash with the library's documentation output.

* Custom migrations may leave a renamed key for the completion step to move.
* Fields removed in a version skipped by a custom migration are dropped.
* Nested named types are checked at the version in flight, not the latest version.
* `default = "..."`: string escapes, exponent notation parses as a float, qualified variant paths.
* The derive reports 128-bit integers, arrays and tuples as compile errors.
* The UIR decoder no longer reserves memory from an unchecked length prefix.

### Added

* `tpt-model`: the schema graph (`Schema`, `TypeDef`, `TypeRef`, `Field`, `Variant`) and the
  `Model` trait, with schema ids computed in a `const fn`.
* `tpt-model`: scoped `Registry` with version chains, graph checks (dangling references and cycles),
  and an optional process-wide registry behind the `global-registry` feature.
* `tpt-model`: the evolution engine. Migration planning with gap, ambiguity, minimum-version and
  downgrade detection; automatic structural evolution (defaults, renames, widening, removal); programmatic
  `Migration` steps with per-step validation.
* `tpt-model`: `ModelEnvelope` with strict bounds checks and a tamper hash, `EntityId` (UUIDv7),
  and the `load` / `save` pipeline.
* `tpt-model`: serde bridge to and from the IR tree, and field-level access rules
  (`intent`, `capability`) with redaction.
* `tpt-model`: JSON and YAML schema export for `tpt-validex`.
* `tpt-model-derive`: `#[derive(Model)]` with `version`, `name`, `since`, `removed_in`, `migrate`,
  `default`, `rename`, `intent` and `capability`. Compile-time checks for version ranges, removed
  fields without `migrate`, unknown attributes, and migration function signatures.
* `tpt-model-cli`: `tpt-model squash` plans removal of versions below a minimum supported version,
  refuses when kept code still uses a removed type, and writes a patch or applies it.
* Local stand-in crates for `tpt-uir`, `tpt-validex`, `tpt-crypto`, `tpt-primitives` and `tpt-testkit`.
* Tests: evolution matrix (V1 to V10), failure scenarios, golden exports, property tests, compile-fail
  tests for the derive, CLI tests, and fuzz targets.
* Example: `cad_part`, an end-to-end CadPart V1 to V2 migration.
* Documentation: versioning rules and the migration author guide.
* CI: formatting, clippy, tests on Linux, Windows and macOS, no_std builds for `thumbv7em` and
  `wasm32`, `cargo deny`, Miri, fuzz builds, and benchmark compilation.

### Notes

* Schema ids use a non-cryptographic 256-bit digest so that they can be computed as constants.
  Tamper detection uses BLAKE3 through the `tpt-crypto` trait.
* Global registration is explicit. There is no link-time auto-registration.
