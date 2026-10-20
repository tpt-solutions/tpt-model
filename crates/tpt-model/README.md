# tpt-model

Schema definition, registry and structural evolution for versioned data. Data written by an
older release of your types is validated, migrated one version at a time, and hydrated into the
current Rust struct, without application code needing to know which version it came from.

* **Pure Rust.** No C-FFI in the shipped dependency graph.
* **`no_std` + `alloc`.** The core builds with `--no-default-features`.
* **Deterministic.** The same value produces the same bytes and the same schema id on every platform.
* **Typed migrations.** Each version is a real Rust struct. Removed data must be handled by a
  migration function, or the derive refuses to compile.

Part of the [tpt-model workspace](https://github.com/tpt-solutions/tpt-model).

## Installation

```sh
cargo add tpt-model
```

The derive macro is re-exported, so `tpt-model` is the only dependency you need for models.
You also need a digest and validator for the load pipeline; the workspace provides local stand-ins
(`tpt-crypto-stub`, `tpt-validex-stub`) until the real TPT crates are available.

## Quick start

Define one struct per version. Each struct says which version it is and what changed.

```rust
use serde::{Deserialize, Serialize};
use tpt_model::{EntityId, Model};

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct CadPartV1 {
    id: EntityId,
    name: String,
    #[tpt(removed_in = 2, migrate = "migrate_legacy_weight")]
    weight_kg: f32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2)]
struct CadPartV2 {
    id: EntityId,
    name: String,
    #[tpt(since = 2)]
    mass_lbs: f64,
}
```

Then register the versions, write a migration, save with the old build, and load with the new one.
The complete program, with the migration function and the save and load calls, is
[`examples/quickstart.rs`](examples/quickstart.rs). Run it with:

```sh
cargo run -p tpt-model --example quickstart
```

The [`cad_part`](examples/cad_part.rs) example adds tamper rejection and current-version round trips.

## Concepts

| Term | Meaning |
|------|---------|
| **Version** | A `#[tpt(version = N)]` struct. All versions of a type share a logical name (`CadPart`, from `CadPartV1`). |
| **Schema graph** | The field, variant and type description of one version, built as `const` data by the derive. |
| **`SchemaId`** | A 256-bit identity of one schema version, computed in a `const fn`. Field order does not change it. |
| **`Registry`** | Holds the schemas, indexed by id and by name and version. It checks the graph for dangling references and cycles. |
| **`Migration`** | A programmatic step from one version to the next, with `From`, `To` and `migrate`. |
| **Automatic evolution** | Adding a field with a `default`, renaming, widening a numeric type, or dropping a field marked `removed_in` with a `migrate`. |
| **`EntityId`** | A UUIDv7 identifier: time-sortable, and rejected unless it is version 7. |
| **Envelope** | The stored form: magic, format, schema id, version, payload and a tamper hash. |

## The load pipeline

`Loader::load::<T>(bytes)` runs these steps, and any failure is a typed error, never a panic:

1. Envelope read, with strict lengths, the magic bytes, and no trailing bytes.
2. Tamper check. The envelope hash is recomputed and compared in constant time.
3. Validation of the raw payload (`tpt-validex-stub` in this workspace).
4. Bytes to the IR tree (`UirNode`).
5. Validation of the tree against the schema of the version it was stored with (`LoadError::SourceShape`).
6. Migration plan from the stored version to the current one. Gaps, ambiguity and versions below
   `min_supported` are errors.
7. Each step runs (a registered migration or the automatic step), and its output is validated against the target schema.
8. Hydration of the tree into `T`.

`save(&digest, id, &value)` produces the envelope for the current version.

## Field attributes

| Attribute | Effect |
|-----------|--------|
| `since = N` | Field exists from version N (default 1). |
| `removed_in = N` | Field is gone from version N. **Requires `migrate`**, unless a later version's migration handles it. |
| `migrate = "path::fn"` | Migration function for the removed data. |
| `default = "expr"` | Value used when automatic evolution adds the field. Literals, strings and qualified enum variants are supported. |
| `rename = "old"` | Former name. May be repeated. |
| `min = N`, `max = N` | Value bounds, or length bounds for strings, bytes and sequences. Enforced at any struct depth. |
| `intent`, `capability` | Field access rule, enforced by `tpt_model::access`. |

## Feature flags

| Feature | Default | Adds |
|---------|---------|------|
| `std` | yes | Standard-library conveniences, the snapshot helpers. The core is `no_std` without it. |
| `global-registry` | no | A process-wide `registry::global` registry. Registration stays explicit; there is no link-time auto-registration. |

Check the no_std build with:

```sh
cargo build -p tpt-model --no-default-features --target thumbv7em-none-eabihf
```

## Tamper evidence

`Blake3` (the default digest) detects **corruption**. Anyone who can rewrite stored bytes can
recompute their hash too. For **forgery** detection, use `KeyedBlake3` from `tpt-crypto-stub` with a
secret key. See [`examples/keyed_digest.rs`](examples/keyed_digest.rs).

`SchemaId` is not a cryptographic hash. It identifies a schema shape. Tamper detection uses the
digest from `tpt-crypto`.

## Design limits

* Constraints (`min`, `max`) do not apply to enum payloads or sequence items, because a bound
  on an item would change the schema id. Check those bounds in code.
* Access rules are enforced when the caller invokes `tpt_model::access::check`. The engine does not
  check on every read.
* Limits on untrusted input: nesting depth 64, strings and sequences up to 16 MiB, payloads up to 64 MiB.

## Tooling

* `check::breaking_changes` compares two registries and lists breaking changes. Use it in CI.
* `evolve::explain` shows the migration plan for two versions without running it.
* `export::json_schema`, `export::typescript`, and `export::{to_json, to_yaml}` export the schema graph.
* `assert_evolves!` checks in a test that a saved value loads as a newer version.
* `assert_schema_snapshot!` (with `std`) guards the schema against unintended changes.
* [`tpt-model-cli`](../tpt-model-cli) removes versions below a minimum supported version and inspects stored entities.

## Stability

Pre-1.0. Breaking changes are recorded in the [changelog](CHANGELOG.md). The schema-id rules are
in [`docs/VERSIONING.md`](../../docs/VERSIONING.md), and the migration author guide is in
[`docs/MIGRATION_GUIDE.md`](../../docs/MIGRATION_GUIDE.md).

## Minimum supported Rust version

1.90, checked in CI.

## Documentation

* [Five-minute tutorial](../../docs/TUTORIAL.md)
* [Cookbook](../../docs/COOKBOOK.md): rename, split, merge, widen, enum changes, and when to migrate
* [Error FAQ](../../docs/FAQ.md): what each error means
* [Versioning rules](../../docs/VERSIONING.md)
* [Migration author guide](../../docs/MIGRATION_GUIDE.md)

## Licence

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
