# tpt-model

Schema definition, registry and structural evolution for the TPT ecosystem. Persisted data
written by an old release is validated, migrated step by step, and hydrated into the current
Rust struct, without the application knowing it came from an old file.

* Pure Rust, no C-FFI in the shipped dependency graph.
* `no_std` + `alloc` for the core crate (`--no-default-features`).
* Deterministic: the same input produces the same bytes on every platform.

## Quickstart

```rust
use serde::{Deserialize, Serialize};
use tpt_model::{EntityId, Model};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
enum Material {
    Steel,
    Titanium,
}

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
    #[tpt(since = 2, default = "Material::Titanium")]
    material: Material,
    #[tpt(since = 2)]
    mass_lbs: f64,
}
```

Save with the old build, load with the new one:

```rust
let bytes = tpt_model::load::save(&Blake3, id, &old_part)?;   // written by v1
let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
let part: CadPartV2 = loader.load(&bytes)?;                  // read by v2
```

The full, runnable version is in [`crates/tpt-model/examples/cad_part.rs`](crates/tpt-model/examples/cad_part.rs):

```sh
cargo run -p tpt-model --example cad_part
```

## Crates

| Crate | Role | `no_std` |
|-------|------|----------|
| [`tpt-model`](crates/tpt-model) | Schema graph, registry, evolution engine, envelope, load pipeline | yes |
| [`tpt-model-derive`](crates/tpt-model-derive) | `#[derive(Model)]` | proc macro |
| [`tpt-model-cli`](crates/tpt-model-cli) | `tpt-model squash`: plan and apply removal of old versions | std |
| `tpt-uir-stub` | Stand-in for `tpt-uir`: IR tree and canonical byte encoding | yes |
| `tpt-validex-stub` | Stand-in for `tpt-validex`: byte-level validation | yes |
| `tpt-crypto-stub` | Stand-in for `tpt-crypto`: digest trait, BLAKE3 (pure Rust) | yes |
| `tpt-primitives-stub` | Stand-in for `tpt-primitives`: `Intent` and `Capability` | yes |
| `tpt-testkit-stub` | Stand-in for `tpt-testkit`: golden-file helpers | std (tests only) |

The `*-stub` crates are local stand-ins for TPT crates that are not yet available. Swapping in
the real crates means replacing their trait implementations; the public API of `tpt-model` stays the same.

## Load pipeline

1. Envelope read (strict lengths, magic, no trailing bytes).
2. Tamper check: the envelope hash is recomputed and compared in constant time.
3. Validation of the raw payload against the source schema.
4. Bytes to IR tree.
5. Migration plan from the stored version to the current one. Gaps, ambiguity and versions below
   `min_supported` are errors.
6. Each step runs (a registered migration, or the automatic structural step), and its output is
   validated against the target schema.
7. Hydration of the IR tree into the Rust struct.

## Commands

```sh
cargo test --workspace                       # all tests
cargo run -p tpt-model --example cad_part    # end-to-end example
cargo bench -p tpt-model --bench migration   # migration throughput
cargo clippy --workspace --all-targets
cargo deny check                             # licences, bans, sources, advisories
cargo build -p tpt-model --no-default-features --target thumbv7em-none-eabihf

cargo run -p tpt-model-cli -- squash --min-supported 2 --patch squash.diff src/
```

Golden files are refreshed with `TPT_UPDATE_GOLDEN=1 cargo test -p tpt-model --test golden`.
Compile-error snapshots are refreshed with `TRYBUILD=overwrite cargo test -p tpt-model --test ui`.

Fuzz targets live in [`fuzz/`](fuzz). They need Linux or macOS with `cargo-fuzz`:
`cargo fuzz run --fuzz-dir fuzz uir_decode`.

## Documentation

* [Versioning rules](docs/VERSIONING.md)
* [Migration author guide](docs/MIGRATION_GUIDE.md)
* [Design spec](spec.txt) and [implementation plan](TODO.md)
* [Changelog](CHANGELOG.md)

## Design notes

* **Schema ids** are a 256-bit digest computed in a `const fn`, so `Model::SCHEMA_ID` is a true
  constant. The digest is not cryptographic. It identifies a schema; tamper detection uses the
  BLAKE3 digest from `tpt-crypto`. Field order does not affect the id.
* **Global registry** (`global-registry` feature) is populated explicitly with
  `registry::global::register::<T>()`. There is no link-time auto-registration.
* **Field access** (`intent`, `capability`) is enforced by `tpt_model::access`. The caller
  decides when to check; the engine does not check on every read.

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
