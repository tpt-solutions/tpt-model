# Five-minute tutorial

This walks through one change: a type gains a field, loses a name, and stops being a plain
string. You will save data with the old shape and read it with the new one.

Everything here runs. The complete program is
[`crates/tpt-model/examples/quickstart.rs`](../crates/tpt-model/examples/quickstart.rs).

## 1. Describe the type you already ship

Every version of a type is its own Rust struct, with `#[derive(Model)]` and a version number:

```rust
#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct CadPartV1 {
    id: EntityId,
    name: String,
    weight_kg: f32,
}
```

The logical name is the struct name with its `V<n>` suffix removed, so this is `CadPart`
version 1. Set `name = "..."` to choose the name yourself.

## 2. Save with it

```rust
let stored = tpt_model::load::save(&Blake3, id, &part_v1)?;
```

`stored` is the envelope: schema id, version, entity id, payload, and a hash. Write it wherever
the old release wrote its files.

## 3. Change the type

Add the new version as a new struct. Mark what the old one loses:

```rust
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

The field `weight_kg` is gone from version 2, and its data needs a home. The old struct says so
with `removed_in` and names a `migrate` function. The derive refuses to compile without the
`migrate` name, so nothing disappears silently.

```rust
#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct CadPartV1 {
    id: EntityId,
    name: String,
    #[tpt(removed_in = 2, migrate = "migrate_legacy_weight")]
    weight_kg: f32,
}
```

## 4. Write the one migration that is not automatic

`material` gets its default and needs no code. `weight_kg` becomes `mass_lbs`, which is a unit
conversion, so it does need code:

```rust
fn migrate_legacy_weight(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    let kg = match node.remove("weight_kg") {
        Some(UirNode::Float(v)) => v,
        _ => return Err(MigrationError::custom("weight_kg must be a float")),
    };
    node.insert("mass_lbs", UirNode::Float(kg * 2.204_62));
    Ok(node)
}
```

Register it, so the engine knows about the step:

```rust
let mut registry = Registry::new();
let mut migrations = Migrations::new();
migrations.register::<CadPartStep>(&mut registry)?;
```

`CadPartStep` is a small `impl Migration` whose `migrate` calls the function above. Both versions
are registered by that call.

## 5. Read old data with the new code

```rust
let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
let part: CadPartV2 = loader.load(&stored)?;
```

The loader checks the envelope hash, checks the stored tree against version 1, runs the steps,
checks the result against version 2, and builds the struct. A tampered or truncated file fails
at the first check, before any step runs.

## 6. Check the plan before you ship it

```sh
tpt-model new-migration --name CadPart crates/          # scaffolds the next version
tpt-model squash --min-supported 2 crates/              # plans removal of version 1
```

For a single evolution, `tpt_model::evolve::explain` returns the same plan as text, field by
field, without running it.

## Next

* [Cookbook](COOKBOOK.md): rename, split, merge, widen, add a variant, and how to choose between
  automatic evolution and a migration.
* [FAQ](FAQ.md): what each error means and how to fix it.
* [Versioning rules](VERSIONING.md) and the [migration author guide](MIGRATION_GUIDE.md).
