# Migration author guide

This guide covers writing a migration when a release changes a persisted type. For the
version rules, see [VERSIONING.md](VERSIONING.md).

Written for: Rust developers who own a persisted type and need to change its shape.

## When you need a migration

Most changes need no code:

| Change | Handled by |
|--------|-----------|
| New field with a `default` or `Option` type | Automatic |
| Renamed field (with `rename = "old"`) | Automatic |
| Widening type change (`i64` to `f64`, integer to a wider integer) | Automatic |
| Field removed and its data is not needed | Migration that drops the key (`node.remove("field")`) |
| Field removed and its data is needed elsewhere | Migration |
| Narrowing or incompatible type change | Migration |
| Value derived from several fields | Migration |

The derive refuses to compile a `removed_in` without a `migrate`, so you cannot forget. Even when
the data is not needed, the removal is explicit: the migration says it is being dropped.

## Step by step

Take a `CadPart` whose weight in kilograms becomes a mass in pounds.

**1. Keep the old struct, and mark its removed field.**

```rust
#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
pub struct CadPartV1 {
    pub id: EntityId,
    pub name: String,
    #[tpt(removed_in = 2, migrate = "migrate_legacy_weight")]
    pub weight_kg: f32,
}
```

**2. Write the new struct.**

```rust
#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2)]
pub struct CadPartV2 {
    pub id: EntityId,
    pub name: String,
    #[tpt(since = 2)]
    pub mass_lbs: f64,
}
```

**3. Write the migration function.** The derive checks that `migrate_legacy_weight`
exists with this signature:

```rust
pub fn migrate_legacy_weight(old: &UirNode, ctx: &MigrationContext)
    -> Result<UirNode, MigrationError>
{
    let mut node = old.clone();
    let kg = match node.remove("weight_kg") {
        Some(UirNode::Float(v)) => v,
        _ => return Err(MigrationError::custom("weight_kg must be a float")),
    };
    node.insert("mass_lbs", UirNode::Float(kg * 2.204_62));
    Ok(node)
}
```

**4. Implement `Migration` for the step.**

```rust
struct CadPartStep;

impl Migration for CadPartStep {
    type From = CadPartV1;
    type To = CadPartV2;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        migrate_legacy_weight(old, ctx)
    }
}
```

**5. Register it.** Registering the migration also registers both of its schemas.

```rust
let mut registry = Registry::new();
let mut migrations = Migrations::new();
migrations.register::<CadPartStep>(&mut registry)?;
registry.check_graph()?;
```

**6. Test it.** Save a value as the old version, and load it as the new one. The
`examples/cad_part.rs` file in this repository is a complete, runnable version.

## What the engine does with your function

For a step with a registered migration:

1. Your function receives the **whole** old tree, including fields it is about to remove.
2. The result is completed automatically. Target fields you did not set are filled from
   their `default` or `Option` rule, and renames apply. Source fields that the target removed are
   dropped.
3. The completed tree is validated against the target schema. A mismatch fails the load
   with `InvalidResult`.

So your function only has to produce the data that is genuinely new. It does not have to
write defaults for unchanged or defaulted fields.

## Working with the IR

The migration sees a `UirNode` tree. It is not the Rust struct, so know these shapes:

| Rust type | IR node |
|-----------|---------|
| `bool` | `Bool` |
| signed integers | `Int` |
| unsigned integers | `UInt` |
| `f32`, `f64` | `Float` |
| `String` | `Str` |
| `Vec<u8>` | `Bytes` |
| `Vec<T>` | `Seq` |
| `Option::None` | `Null` |
| structs and maps | `Map` (in declaration order) |
| unit enum variant | `Str(variant name)` |
| newtype enum variant | `Map([(variant name, payload)])` |
| `EntityId` | `Bytes` (16 bytes) |

Two details trip people up:

* A default such as `"0"` is an `Int`, even for a `u32` field. Accept both `Int` and
  `UInt` in migration code that reads numbers.
* Map keys are strings. `node.get`, `node.insert` and `node.remove` work on top-level
  keys only. Nested values are ordinary `UirNode`s you can match on.

## Default expressions

`default = "..."` is evaluated when a field is filled in:

| Expression | Value |
|------------|-------|
| `null`, `None` | `Null` |
| `true`, `false` | `Bool` |
| `42`, `-7` | `Int` (or `UInt` above `i64::MAX`) |
| `1.5`, `-0.25` | `Float` |
| `"text"` | `Str` |
| `Material::Titanium` | `Str("Titanium")` (the variant name) |
| `[]` | empty `Seq` |
| `{}` | empty `Map` |

Anything else is a `Default` error at load time, not a panic.

## Errors

Return `MigrationError::custom("message")` for failures in your own code. The engine
returns the other errors itself:

| Error | Usually means |
|-------|---------------|
| `Gap` | A version between source and target is not registered |
| `Ambiguous` | Two equally short chains of migrations exist. Remove one |
| `NoPath` | No chain connects the two versions |
| `BelowMinSupported` | The data is older than `min_supported` |
| `MissingField` | A new field has no default and is not an `Option` |
| `RequiresMigration` | A removed field with `migrate` has no registered migration |
| `UnknownField` | The source has a field the target does not know |
| `TypeChange` | A field changed type in a non-widening way |
| `InvalidResult` | Your migration returned a tree that does not match the target |

## Checklist

* [ ] Old struct has `removed_in` and `migrate` on every removed field.
* [ ] New fields have `default` or are `Option`.
* [ ] Migration function signature matches `(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError>`.
* [ ] Migration is registered, and `registry.check_graph()` passes.
* [ ] A test saves the old version and loads the new one.
* [ ] `min_supported` is set if older versions should be refused.
