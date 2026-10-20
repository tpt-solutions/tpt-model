# tpt-model

Schema definition, registry and structural evolution for the TPT ecosystem. Persisted data
written by an old release is validated, migrated step by step, and hydrated into the current
Rust struct, without the application knowing it came from an old file.

* Pure Rust, no C-FFI in the shipped dependency graph.
* `no_std` + `alloc` for the core crate (`--no-default-features`).
* Deterministic: the same input produces the same bytes on every platform.
* Minimum supported Rust version: 1.90 (checked in CI).

## Quickstart

This is [`crates/tpt-model/examples/quickstart.rs`](crates/tpt-model/examples/quickstart.rs),
complete. Run it with `cargo run -p tpt-model --example quickstart`.

```rust
use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Model)]
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

/// Turns the removed `weight_kg` into `mass_lbs`.
fn migrate_legacy_weight(
    old: &UirNode,
    _ctx: &MigrationContext,
) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    let kg = match node.remove("weight_kg") {
        Some(UirNode::Float(v)) => v,
        _ => return Err(MigrationError::custom("weight_kg must be a float")),
    };
    node.insert("mass_lbs", UirNode::Float(kg * 2.204_62));
    Ok(node)
}

struct CadPartStep;

impl Migration for CadPartStep {
    type From = CadPartV1;
    type To = CadPartV2;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        migrate_legacy_weight(old, ctx)
    }
}

fn main() {
    let mut registry = Registry::new();
    registry.register::<Material>().expect("register Material");
    let mut migrations = Migrations::new();
    migrations
        .register::<CadPartStep>(&mut registry)
        .expect("register migration");

    let id = EntityId::new_v7(1_700_000_000_000, [7; 10]);

    // Written by the old build.
    let stored = save(
        &Blake3,
        id,
        &CadPartV1 {
            id,
            name: "bracket".into(),
            weight_kg: 2.0,
        },
    )
    .expect("save");

    // Read by the new build.
    let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
    let part: CadPartV2 = loader.load(&stored).expect("load");
    println!("{part:?}");
    assert_eq!(part.material, Material::Titanium);
}
```

The `cad_part` example adds the rest of the flow: a tampered payload is rejected, and
current-version data round-trips without a migration.

```sh
cargo run -p tpt-model --example cad_part
```

## Crates

| Crate | Role | `no_std` |
|-------|------|----------|
| [`tpt-model`](crates/tpt-model) | Schema graph, registry, evolution engine, envelope, load pipeline | yes |
| [`tpt-model-derive`](crates/tpt-model-derive) | `#[derive(Model)]` | proc macro |
| [`tpt-model-cli`](crates/tpt-model-cli) | `tpt-model squash` (plan and apply removal of old versions), `tpt-model inspect` (envelope and tree of a stored entity) and `tpt-model new-migration` (scaffold the next version); also `cargo tpt-model` | std |
| `tpt-uir-stub` | Stand-in for `tpt-uir`: IR tree and canonical byte encoding | yes |
| `tpt-validex-stub` | Stand-in for `tpt-validex`: byte-level validation | yes |
| `tpt-crypto-stub` | Stand-in for `tpt-crypto`: digest trait, BLAKE3 (pure Rust) | yes |
| `tpt-primitives-stub` | Stand-in for `tpt-primitives`: `Intent` and `Capability` | yes |
| `tpt-testkit-stub` | Stand-in for `tpt-testkit`: golden-file helpers | std (tests only) |

The `*-stub` crates are local stand-ins for TPT crates that are not yet available. Swapping in
the real crates means replacing their trait implementations; the public API of `tpt-model` stays the same.

Each crate has its own README and CHANGELOG. To start a new model crate, use the
[`cargo-generate` template](templates/model-crate) (`cargo generate --path templates/model-crate`).

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

cargo run -p tpt-model --example quickstart        # the README quickstart, complete
cargo run -p tpt-model --example keyed_digest      # tamper evidence with a keyed digest
cargo run -p tpt-model --example config_file       # automatic and registered steps across versions
cargo run -p tpt-model --example custom_migration  # lossy type change (float to integer)
cargo run -p tpt-model --example rename_and_split  # one field becomes two
cargo run -p tpt-model --example save_game         # enum variant evolution

cargo run -p tpt-model-cli -- squash --min-supported 2 --patch squash.diff src/
cargo run -p tpt-model-cli -- squash --min-supported 2 --json src/   # plan as JSON
cargo run -p tpt-model-cli -- inspect path/to/entity.bin            # envelope and tree
cargo run -p tpt-model-cli -- new-migration --name CadPart src/part.rs
cargo tpt-model --help                                              # after installing the CLI
```

Golden files are refreshed with `TPT_UPDATE_GOLDEN=1 cargo test -p tpt-model --test golden`.
Compile-error snapshots are refreshed with `TRYBUILD=overwrite cargo test -p tpt-model --test ui`.
Schema snapshots in a generated model crate are refreshed with `TPT_UPDATE_SNAPSHOTS=1 cargo test`.

Fuzz targets live in [`fuzz/`](fuzz). They need Linux or macOS with `cargo-fuzz`:
`cargo fuzz run --fuzz-dir fuzz uir_decode`.

## Documentation

* [Five-minute tutorial](docs/TUTORIAL.md)
* [Cookbook](docs/COOKBOOK.md): rename, split, merge, widen, enum changes, and when to migrate
* [Error FAQ](docs/FAQ.md): what each error means and how to fix it
* [Versioning rules](docs/VERSIONING.md)
* [Migration author guide](docs/MIGRATION_GUIDE.md)
* [Contributing](CONTRIBUTING.md) and [security policy](SECURITY.md)
* [Design spec](spec.txt) and [implementation plan](TODO.md)
* [Changelog](CHANGELOG.md), and per-crate changelogs in each `crates/*/CHANGELOG.md`

## Design notes

* **Schema ids** are a 256-bit digest computed in a `const fn`, so `Model::SCHEMA_ID` is a true
  constant. The digest is not cryptographic. It identifies a schema; tamper detection uses the
  BLAKE3 digest from `tpt-crypto`. Field order does not affect the id.
* **Tamper evidence.** `Blake3` detects corruption only: anyone who can rewrite stored bytes can
  also recompute their hash. `KeyedBlake3` (from `tpt-crypto-stub`) also rejects forgeries, provided
  the key stays secret. See `examples/keyed_digest.rs`.
* **Constraints** (`min`, `max`) apply to struct fields at any depth, including fields of nested
  structs. They do not apply to enum payloads or to sequence items: a bound on an item would change
  the schema id, so check those bounds in code.
* **Name matching in `squash`** is by name, not by module path. A type that shares its name with
  a removed type in another module can make the tool refuse. That is the safe direction.
* **Global registry** (`global-registry` feature) is populated explicitly with
  `registry::global::register::<T>()`. There is no link-time auto-registration.
* **Field access** (`intent`, `capability`) is enforced by `tpt_model::access`. The caller
  decides when to check; the engine does not check on every read.

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
