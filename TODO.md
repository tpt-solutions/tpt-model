# tpt-model TODO

Source of truth: [spec.txt](spec.txt). Tick boxes as you go.

## Decisions (from clarifying Q&A)
- External TPT crates (tpt-uir, tpt-validex, tpt-crypto, tpt-primitives, tpt-testkit) are **stubbed locally** via traits/mock crates; swap for real ones later.
- **Cargo workspace**: `tpt-model`, `tpt-model-derive`, `tpt-model-cli`, plus stub crates.
- **One real Rust struct per version** (`CadPartV1`, `CadPartV2`); migrations are `impl Migration`; derive emits compile errors for removed fields lacking `migrate`.
- `SchemaId` = BLAKE3-style hash over a canonical, field-order-normalized schema encoding (pure Rust, no_std).
- **Serde <-> UirNode**: custom Serializer/Deserializer targeting the IR tree; byte encoding is tpt-uir's job.
- **Explicit scoped registry** (no_std friendly); optional std-only global auto-registration behind a feature.
- Scope: **all 4 phases**. Squash CLI = **analyze & report** (plan/patch, optional `--apply` later).
- Assumptions (shout if wrong): UUIDv7 via `uuid` crate (no_std features); Rust stable, edition 2021; MIT/Apache-2.0 dual license.

## 0. Workspace setup
- [ ] Create Cargo workspace (`Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `git init`)
- [ ] Crates: `tpt-model`, `tpt-model-derive`, `tpt-model-cli`
- [ ] Stub crates: `tpt-uir-stub`, `tpt-validex-stub`, `tpt-crypto-stub`, `tpt-primitives-stub`, `tpt-testkit-stub`
- [ ] `#![no_std]` + `extern crate alloc` in core; `std` / `global-registry` feature flags
- [ ] CI config (fmt, clippy, test, `thumbv7em`/`wasm32` no_std build check)
- [ ] README with crate map and quickstart
- [ ] Verify zero C-FFI (cargo tree audit / `cargo deny`)

## 1. Phase 1 — Meta-Model & Macros (spec weeks 1-3)
### Schema (`tpt_model::schema`)
- [ ] Schema graph types: Struct, Enum, Union, Map, Array, Option, primitives
- [ ] Field metadata: name, type, since, removed_in, default, rename history, constraints
- [ ] DAG validation (reject cycles, dangling refs)
- [ ] Canonical encoding (order-normalized, deterministic across platforms)
- [ ] `SchemaId` (hash of canonical encoding); pure-Rust hash implementation
- [ ] `Model` trait (`SCHEMA_ID`, `SCHEMA_VERSION`, `schema()`)
- [ ] Scoped `Registry` (`register::<T>()`, lookup by SchemaId, version chain by type name)
- [ ] Optional global auto-registration (std feature)
- [ ] JSON/YAML schema export (for tpt-validex and golden tests)
### Derive (`tpt-model-derive`)
- [ ] `#[derive(Model)]` for structs and enums
- [ ] Attributes: `version`, `since`, `removed_in`, `rename`, `default`, `migrate`
- [ ] Compile-time errors: removed field without `migrate`, bad version ranges, unknown attrs
- [ ] trybuild compile-fail tests
### Entity (`tpt_model::entity`)
- [ ] UUIDv7 `EntityId` newtype (time-sortable, no_std)
- [ ] `ModelEnvelope` (SchemaId, version, payload, hash)
- [ ] Hash/tamper check via crypto trait (stubbed tpt-crypto)
- [ ] Envelope encode/decode with strict bounds checking

## 2. Phase 2 — Automated Evolution (weeks 4-5)
- [ ] Serde Serializer/Deserializer <-> `UirNode`
- [ ] Generic IR tree read of entities
- [ ] Auto-migrations: add field with default, drop field, rename, widening type changes
- [ ] Default expression evaluation (`default = "..."`)
- [ ] Determinism tests (identical output across OS/arch)

## 3. Phase 3 — Programmatic Migrations (weeks 6-8)
- [ ] `Migration` trait (`From`, `To`, `migrate(&UirNode, &MigrationContext)`)
- [ ] `MigrationContext` and `MigrationError` (no panics)
- [ ] Migration registry (register by From/To SchemaId)
- [ ] Path resolver (V1 -> V2 -> ... -> Vn), detect gaps/ambiguity
- [ ] Chained execution with per-step validation
- [ ] `min_supported_version` enforcement with clear error
- [ ] Hydration IR -> Rust struct

## 4. Load pipeline (`load::<T>(bytes)`)
- [ ] Envelope read
- [ ] Tamper-hash check
- [ ] Validation via validex trait using source-version schema
- [ ] Bytes -> UirNode
- [ ] Migration resolution + execution
- [ ] Hydration to current struct
- [ ] End-to-end example (CadPart V1 -> V2 with `migrate_legacy_weight`)

## 5. Phase 4 — Ecosystem Integration (weeks 9-10)
- [ ] Schema export adapter for tpt-validex (cross-language schemas)
- [ ] tpt-primitives `Intent`/`Capability` field-level access attributes (stubbed)
- [ ] tpt-vertex CAD document example/adapter
- [ ] Docs: migration author guide, versioning rules
- [ ] Publish prep: metadata, changelog, v1.0.0 tag

## 6. Squash/Compact CLI (`tpt-model-cli`)
- [ ] Scan source (syn) for versioned models and migrations
- [ ] Accept `--min-supported <N>`
- [ ] Report deletable structs/migrations and affected files
- [ ] Emit plan/patch output; `--apply` (stretch)
- [ ] Refuse if registry still references data needing removed versions

## 7. Testing & Quality
- [ ] Evolution matrix test: V1..V10, step-by-step, no data loss
- [ ] Fuzzing: garbage UirNodes -> `MigrationError`, no panics/UB (cargo-fuzz)
- [ ] Golden-file snapshots of schema JSON/YAML + SchemaId stability
- [ ] Property tests (proptest) for roundtrip + canonical encoding
- [ ] Miri run on core crate
- [ ] `#![forbid(unsafe_code)]` where possible
- [ ] Benchmarks (migration throughput)

## Open items to confirm later
- [ ] Real locations/APIs of tpt-uir, tpt-validex, tpt-crypto, tpt-primitives, tpt-testkit
- [ ] Enum/union evolution rules (variant add/remove/rename)
- [ ] Type-change rules beyond widening (which require explicit migration)
