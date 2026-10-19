# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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
