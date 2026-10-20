# tpt-model TODO

Source of truth: [spec.txt](spec.txt). Tick boxes as you go.

## Decisions (from clarifying Q&A)
- External TPT crates (tpt-uir, tpt-validex, tpt-crypto, tpt-primitives, tpt-testkit) are **stubbed locally** via traits/mock crates; swap for real ones later.
- **Cargo workspace**: `tpt-model`, `tpt-model-derive`, `tpt-model-cli`, plus stub crates.
- **One real Rust struct per version** (`CadPartV1`, `CadPartV2`); migrations are `impl Migration`; derive emits compile errors for removed fields lacking `migrate`.
- `SchemaId` = BLAKE3-style hash over a canonical, field-order-normalized schema encoding (pure Rust, no_std). **Deviation:** the id is a `const fn` sponge digest, because `Model::SCHEMA_ID` must be a `const`. Tamper detection uses BLAKE3 (pure feature) through the `tpt-crypto` trait.
- **Serde <-> UirNode**: custom Serializer/Deserializer targeting the IR tree; byte encoding is tpt-uir's job.
- **Explicit scoped registry** (no_std friendly); optional std-only global auto-registration behind a feature.
- Scope: **all 4 phases**. Squash CLI = **analyze & report** (plan/patch, optional `--apply` later).
- Assumptions (shout if wrong): UUIDv7 via `uuid` crate (no_std features); Rust stable, edition 2021; MIT/Apache-2.0 dual license.

## 0. Workspace setup
- [x] Create Cargo workspace (`Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `git init`)
- [x] Crates: `tpt-model`, `tpt-model-derive`, `tpt-model-cli`
- [x] Stub crates: `tpt-uir-stub`, `tpt-validex-stub`, `tpt-crypto-stub`, `tpt-primitives-stub`, `tpt-testkit-stub`
- [x] `#![no_std]` + `extern crate alloc` in core; `std` / `global-registry` feature flags
- [x] CI config (fmt, clippy, test, `thumbv7em`/`wasm32` no_std build check) — `.github/workflows/ci.yml`
- [x] README with crate map and quickstart
- [x] Verify zero C-FFI (cargo tree audit / `cargo deny`) — `deny.toml`. Shipped graph has no `-sys` or C-compiling crates. Exception: blake3 lists `cc` as a build dependency, which compiles nothing under its `pure` feature; the exception is scoped to blake3 in `deny.toml`.

## 1. Phase 1 — Meta-Model & Macros (spec weeks 1-3)
### Schema (`tpt_model::schema`)
- [x] Schema graph types: Struct, Enum, Union, Map, Array, Option, primitives
- [x] Field metadata: name, type, since, removed_in, default, rename history, constraints (`min` / `max`; value bounds, or length for strings, bytes and sequences)
- [x] DAG validation (reject cycles, dangling refs) — `Registry::check_graph`
- [x] Canonical encoding (order-normalized, deterministic across platforms)
- [x] `SchemaId` (hash of canonical encoding); pure-Rust hash implementation (const-fn digest, see Decisions)
- [x] `Model` trait (`SCHEMA_ID`, `SCHEMA_VERSION`, `schema()`)
- [x] Scoped `Registry` (`register::<T>()`, lookup by SchemaId, version chain by type name)
- [x] Optional global registry (std feature), registered explicitly via `registry::global::register::<T>()`. No automatic registration; see section 8B for link-time registration.
- [x] JSON/YAML schema export (for tpt-validex and golden tests)
### Derive (`tpt-model-derive`)
- [x] `#[derive(Model)]` for structs and enums
- [x] Attributes: `version`, `since`, `removed_in`, `rename`, `default`, `migrate` (plus `name`, `intent`, `capability`, `min`, `max`)
- [x] Compile-time errors: removed field without `migrate`, bad version ranges, unknown attrs
- [x] trybuild compile-fail tests
### Entity (`tpt_model::entity`)
- [x] UUIDv7 `EntityId` newtype (time-sortable, no_std)
- [x] `ModelEnvelope` (SchemaId, version, payload, hash)
- [x] Hash/tamper check via crypto trait (stubbed tpt-crypto)
- [x] Envelope encode/decode with strict bounds checking

## 2. Phase 2 — Automated Evolution (weeks 4-5)
- [x] Serde Serializer/Deserializer <-> `UirNode`
- [x] Generic IR tree read of entities — `tpt_model::load::read_tree`
- [x] Auto-migrations: add field with default, drop field, rename, widening type changes. Drop applies to a field removed without `migrate`. The derive always requires `migrate` on removed fields, so derived types cannot drop silently; see the type-safe evolution rule.
- [x] Default expression evaluation (`default = "..."`)
- [x] Determinism tests (identical output across OS/arch) — pinned encoding bytes and golden exports, checked on Linux, Windows and macOS in CI. 32-bit targets are not exercised.

## 3. Phase 3 — Programmatic Migrations (weeks 6-8)
- [x] `Migration` trait (`From`, `To`, `migrate(&UirNode, &MigrationContext)`)
- [x] `MigrationContext` and `MigrationError` (no panics)
- [x] Migration registry (register by From/To SchemaId)
- [x] Path resolver (V1 -> V2 -> ... -> Vn), detect gaps/ambiguity
- [x] Chained execution with per-step validation
- [x] `min_supported_version` enforcement with clear error
- [x] Hydration IR -> Rust struct

## 4. Load pipeline (`load::<T>(bytes)`)
- [x] Envelope read
- [x] Tamper-hash check
- [x] Validation of the stored tree against its source-version schema (`load.rs`, `LoadError::SourceShape`). **Note:** the validex stub (`BoundsValidator`) still ignores its schema argument; see 8A and 8B.
- [x] Bytes -> UirNode
- [x] Migration resolution + execution
- [x] Hydration to current struct
- [x] End-to-end example (CadPart V1 -> V2 with `migrate_legacy_weight`) — `examples/cad_part.rs`

## 5. Phase 4 — Ecosystem Integration (weeks 9-10)
- [x] Schema export adapter for tpt-validex (cross-language schemas)
- [x] tpt-primitives `Intent`/`Capability` field-level access attributes (stubbed)
- [x] tpt-vertex CAD document example/adapter — the `cad_part` example stands in for tpt-vertex; no real tpt-vertex code exists to integrate with
- [x] Docs: migration author guide, versioning rules — `docs/`
- [ ] Publish prep: metadata, changelog, v1.0.0 tag — metadata and `CHANGELOG.md` are done. **Not done, by request:** the v1.0.0 tag and publishing.

## 6. Squash/Compact CLI (`tpt-model-cli`)
- [x] Scan source (syn) for versioned models and migrations
- [x] Accept `--min-supported <N>`
- [x] Report deletable structs/migrations and affected files
- [x] Emit plan/patch output; `--apply` (stretch)
- [x] Refuse if registry still references data needing removed versions — refuses when kept code refers to a removed type. The tool cannot see persisted data, so the check is over source references only.

## 7. Testing & Quality
- [x] Evolution matrix test: V1..V10, step-by-step, no data loss
- [x] Fuzzing: garbage UirNodes -> `MigrationError`, no panics/UB — property tests run on every `cargo test`; cargo-fuzz targets in `fuzz/` build in CI. Fuzzing runs on Linux/macOS only.
- [x] Golden-file snapshots of schema JSON/YAML + SchemaId stability
- [x] Property tests (proptest) for roundtrip + canonical encoding
- [x] Miri run on core crate — `cargo miri test -p tpt-model --lib`, passing; also in CI
- [x] `#![forbid(unsafe_code)]` where possible — every crate's `lib.rs`
- [x] Benchmarks (migration throughput) — `cargo bench -p tpt-model --bench migration`, about 0.6 ms per 1,000-value load through two steps

## 8. Review follow-ups (bugs, features, adoption)
Added after a code review. See the plan for detail and ordering: Section A first (the `SchemaId` change must land before 1.0).

### A. Bugs and correctness
- [x] Renames after a custom migration: `complete_after_migration` (`evolve.rs`) accepts keys that match a target field's `rename` history. Tests: `tests/custom_migrations.rs`
- [x] Skip migrations: fields removed in any version in `(from, to]` are dropped. Tests: `tests/custom_migrations.rs`
- [x] Nested `TypeRef::Named` resolves at the version in flight (`Registry::at_or_before`, `shape.rs`). Tests: `tests/nested_versions.rs`
- [x] `SchemaId` metadata: documented in `docs/VERSIONING.md` ("What changes a `SchemaId`"). **Not changed:** excluding `migrate`/`default`/`capability` from the hash is a pre-1.0 decision, left to you
- [x] Decode memory amplification: pre-allocation capped at 1024 elements in `tpt-uir-stub`. Test: `collections_larger_than_the_preallocation_limit_round_trip`
- [x] Squash CLI: preserve CRLF on `--apply` (lines keep their terminators)
- [x] Squash CLI: verify the unified-diff hunk header and context with `git apply --check`. Patches now carry three lines of context; the old zero-context hunks were anchored at the top of the file and did not apply. Tests: `tests/patch_and_apply.rs`
- [x] Squash CLI: name-only matching. **Decided: keep it.** Matching is by name, so it errs toward refusal (it never removes a type that code still uses under the same name). The limit is documented in `docs/FAQ.md`. Module-path resolution is not planned
- [x] Squash CLI: `--apply` restores already-written files if a later write fails; malformed `#[tpt]` and `#[derive]` attributes are errors
- [x] Constraints on nested struct fields (any depth) are enforced (`tests/constraints_nested.rs`). **Decided:** enum payloads and sequence items stay unconstrained, because a bound on an item needs a schema change (new `SchemaId`s). Check those bounds in code. Documented in the README
- [x] Derive: clear compile errors for `u128`/`i128`, arrays and tuples (trybuild cases in `tests/ui/`). **Type aliases** still become `Named` refs; the macro cannot see through them, so they are caught only by `Registry::check_graph`
- [x] `EntityId`: `try_from_bytes` and `Deserialize` accept only UUIDv7; `generate` helper; deserialization works from self-describing sequences
- [x] `default_expr`: string escapes, `1e5` parses as a float, qualified variant paths
- [x] `redact` covers nested structs, enum payloads and sequences; it now takes a `Registry`, and unresolvable nested types fail closed (`null`)
- [x] Registry lookups use a name index (`by_name`)
- [x] `BoundsValidator` decodes the payload again after validating it. **Decided:** keep both until `tpt-validex` works on raw bytes. The loader checks the decoded tree against its source-version schema (`LoadError::SourceShape`), so the schema check is no longer missing
- [x] Correct the overstated ticks above
- [x] Reconcile `Envelope` vs spec's `ModelEnvelope` (type alias `ModelEnvelope = Envelope`)

### B. Missing features
- [x] Shape-aware validator that uses the source-version schema. Done in `load` (`LoadError::SourceShape`), not in the validex stub
- [x] Keyed digest example for tamper evidence; document that unkeyed BLAKE3 detects corruption only. `KeyedBlake3` in `tpt-crypto-stub`, `examples/keyed_digest.rs`, `tests/tamper.rs`, README and SECURITY.md
- [x] Link-time auto-registration: **Decided: not implemented.** Registration stays explicit (`registry.register::<T>()`, `registry::global::register::<T>()`), documented in the README design notes. It avoids a linker-section dependency
- [x] JSON Schema / TypeScript export: `export::json_schema` and `export::typescript`, tested in `tests/tooling.rs` and `tests/export_typescript.rs`. Validation against the JSON Schema meta-schema is not run in tests

### C. New CLI commands
- [x] `tpt-model check` (library): `check::breaking_changes`, tested in `tests/tooling.rs`. **Decided:** the supported form is the library, used from tests or CI. A CLI command would need a snapshot reader, and the CLI cannot load compiled types
- [x] `snapshot`: **Decided:** the test helper is the supported form, `assert_schema_snapshot!` and `snapshot::check` (`tests/snapshot.rs`, `tests/derive_snapshot.rs`). A CLI command cannot load compiled types
- [x] `tpt-model inspect FILE`: envelope header, schema id, version, tree dump; `--key-hex` for keyed data (`tests/driver.rs`)
- [x] `explain` (library): `evolve::explain`, tested in `tests/tooling.rs`. **Decided:** library only, for the same reason as `check`
- [x] `tpt-model new-migration --name TYPE PATH...`: prints the next version (a copy of the latest, bumped) with a migration stub. Tests in `tests/driver.rs`
- [x] `cargo tpt-model` shim (second binary, `cargo-tpt-model`)
- [x] Real `--help` and `--json` output (`squash --json`). **Not done:** shell completions

### D. Adoption: docs, examples, templates
- [x] Make the README quickstart compile: `examples/quickstart.rs` is the README code, and CI runs it
- [x] Examples: `config_file` (v1 to v3), `save_game` (enum evolution), `rename_and_split`, `custom_migration`, all in `crates/tpt-model/examples/` and run in CI. `squash_demo/` is done
- [x] `cargo-generate` template: `templates/model-crate/`. Verified by substituting the placeholders and running its tests in a scratch copy. `cargo-generate` itself was not run
- [x] Round-trip testkit macro: `assert_evolves!` is exported from `tpt-model`, not from the testkit stub (which would create a dev-dependency cycle). Generated proptests: `tests/evolution_proptest.rs`
- [x] Docs: `docs/TUTORIAL.md`, `docs/COOKBOOK.md` (with the migrate-versus-automatic table), `docs/FAQ.md` (every error message)
- [x] CONTRIBUTING.md, SECURITY.md; `docs/VERSIONING.md` covers what changes `SchemaId`
- [ ] Install path: publishable CLI or prebuilt binaries; keep stub crates off crates.io. **Excluded:** publishing, by request
- [x] Fix `docs/MIGRATION_GUIDE.md` claim that removal without `migrate` is "not allowed"

### E. CI and release automation
- [x] Run CI on all branches (the `push` trigger has no branch filter now)
- [x] Jobs: `global-registry` tests, `--no-default-features` tests, `cargo doc -D warnings` (`feature-matrix` and `docs` jobs), CLI run against `squash_demo`, MSRV (`rust-version = "1.90"`, checked locally with 1.90.0). Coverage job added (report only, uploads lcov). Verified locally with `cargo llvm-cov`: 78% of lines in `tpt-model`. A 32-bit Windows job runs the core and stub suites. The MSRV is 1.90 because the dev-dependencies and `uuid` need newer compilers than 1.85; going lower means changing dependency versions, which I did not do
- [ ] Release workflow: tag, publish dry run, GitHub release with binaries. **Not started:** tagging and publishing were excluded by request
- [x] Dependabot for cargo and GitHub Actions (`.github/dependabot.yml`). `deny.toml` advisories: explicit ignore list (this cargo-deny has no severity keys; all four checks pass). **Decided:** no renovate, and no toolchain pinning beyond `stable` and the MSRV job. Changelog check job added
- [x] Fuzz target that builds valid envelopes (correct hash) to reach validation and migration: `fuzz/fuzz_targets/envelope_valid.rs`, in CI. It type-checks here; it was not run under libFuzzer (Windows). A deterministic version runs in `tests/valid_envelope_smoke.rs`

### F. Test gaps
- [x] Custom migrations that skip versions or rename; nested shape changes; enum variant add/remove; `SchemaId` sensitivity. `tests/custom_migrations.rs`, `tests/nested_versions.rs`, `tests/schema_id.rs`, `tests/tooling.rs`
- [x] CLI integration tests: exit codes and the binary (`tests/cli_exit.rs`); CRLF; `git apply`; `--json`; `inspect`; `new-migration`; same-line items, `use` and `#[cfg(test)]` modules (`tests/squash_edge.rs`). A shared-line refusal was added: removal works on whole lines, so an item sharing a line with a removed one is now refused
- [x] `global-registry` tests (`tests/global_registry.rs`); derive positive snapshot (`tests/derive_snapshot.rs`); 32-bit: the core and stub suites pass as `i686-pc-windows-msvc` binaries, golden files included, and CI has a job for it

## Open items to confirm later
- [ ] Real locations/APIs of tpt-uir, tpt-validex, tpt-crypto, tpt-primitives, tpt-testkit. **Waiting:** the real crates are not in this repo, so the stubs stay until they exist
- [x] Enum/union evolution rules. **Decided:** additions are automatic (old data reads); removing or renaming a variant needs a migration. Documented in `docs/COOKBOOK.md`
- [x] Type-change rules beyond widening. **Decided:** only widening is automatic; every other change needs an explicit migration. Documented in `docs/COOKBOOK.md`
