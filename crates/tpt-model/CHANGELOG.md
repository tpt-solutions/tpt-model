# Changelog

All notable changes to `tpt-model` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace-wide log is in the
[root changelog](../../CHANGELOG.md).

## [Unreleased]

### Changed (breaking, before 1.0)

* `access::redact` takes a `&Registry` and redacts nested structs, enum payloads and sequences.
  Nested types the registry cannot resolve are replaced by `null`.
* `load` checks the stored tree against its source-version schema before migrating. Failures are
  reported as `LoadError::SourceShape`, which replaces `MigrationError::InvalidResult` for data that
  was already invalid at its own version.
* `EntityId` deserialization accepts only UUIDv7 and works from self-describing formats. Use
  `EntityId::try_from_bytes` for untrusted bytes.
* Keywords and categories are set per crate, and the crate has its own README.

### Added

* Schema graph (`Schema`, `TypeDef`, `TypeRef`, `Field`, `Variant`) and the `Model` trait, with
  schema ids computed in a `const fn`.
* Scoped `Registry` with version chains, graph checks (dangling references and cycles), and an
  optional process-wide registry behind `global-registry`.
* Evolution engine: migration planning with gap, ambiguity, minimum-version and downgrade detection;
  automatic structural evolution (defaults, renames, widening, removal); programmatic `Migration`
  steps with per-step validation.
* `ModelEnvelope` (`Envelope`) with strict bounds checks and a tamper hash; `EntityId` (UUIDv7);
  the `load` / `save` pipeline.
* Serde bridge to and from the IR tree, and field-level access rules (`intent`, `capability`) with redaction.
* JSON and YAML schema export, JSON Schema (draft 2020-12) and TypeScript export.
* `check::breaking_changes` and `check::report`: a breaking-change linter over two registries.
* `evolve::explain`: the plan for a pair of versions, field by field, without running it.
* `assert_evolves!` and `assert_schema_snapshot!` for downstream tests.
* Examples: `quickstart`, `cad_part`, `config_file`, `save_game`, `rename_and_split`,
  `custom_migration`, `keyed_digest`. Benchmark: `migration`.

### Fixed

* Custom migrations may leave a renamed key for the completion step to move.
* Fields removed in a version skipped by a custom migration are dropped.
* Nested named types are checked at the version in flight, not the latest version.
* `default = "..."`: string escapes, exponent notation parses as a float, qualified variant paths.
* The derive reports 128-bit integers, arrays and tuples as compile errors.

## [0.1.0]

Not released yet. The first release will be tagged after the v1.0.0 decision.
