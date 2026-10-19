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
- [x] Optional global auto-registration (std feature) — **explicit** `registry::global::register::<T>()`. Link-time auto-registration is not implemented; it needs a linker-section crate.
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
- [x] Validation via validex trait using source-version schema
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

## Open items to confirm later
- [ ] Real locations/APIs of tpt-uir, tpt-validex, tpt-crypto, tpt-primitives, tpt-testkit
- [ ] Enum/union evolution rules (variant add/remove/rename). **Current behaviour:** enum evolution is the identity, and an unknown variant fails shape validation. Variant rename is not supported.
- [ ] Type-change rules beyond widening (which require explicit migration). **Current behaviour:** only widening is automatic; every other change fails with `TypeChange`.
