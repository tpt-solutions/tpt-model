# Cookbook

Each recipe names the example or test that exercises it, so the code shown here is the code
that runs.

## Automatic or migration?

| Change | Automatic | Migration needed |
|--------|-----------|------------------|
| Field added with a `default` | yes | |
| Field added as `Option<T>` | yes (becomes `null`) | |
| Field added without a default or `Option` | no (error) | yes, or give it a default |
| Field renamed (`rename = "old"`) | yes | |
| Widening (`i64` or `u64` to `f64`) | yes | |
| Field removed, data not needed | | yes, a migration that drops the key |
| Field removed, data kept elsewhere | | yes |
| Field split into two, or two merged into one | | yes |
| Narrowing or lossy type change (`f64` to `i64`) | | yes |
| Enum variant added | yes (old data still reads) | |
| Enum variant removed or renamed | | yes |
| Changed meaning with the same type (units, encodings) | | yes |

A rule of thumb: if the new code can compute the new value from the stored value without
choosing how, it is automatic. If someone has to decide, write a migration.

Everything automatic is listed in the [migration guide](MIGRATION_GUIDE.md). `explain` shows the
exact steps for a given pair of versions.

## Rename a field

Use `rename`. No migration is needed.

```rust
#[derive(Serialize, Deserialize, Model)]
#[tpt(version = 2)]
struct ConfigV2 {
    #[tpt(rename = "server")]
    host: String,
}
```

Covered by `tests/evolution_matrix.rs` and the `config_file` example.

## Split one field into two

Splitting needs a migration, because the automatic steps cannot guess the division. Mark the old
field `removed_in` with a `migrate` function on the old struct.

```rust
#[derive(Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct PersonV1 {
    #[tpt(removed_in = 2, migrate = "split_name")]
    full_name: String,
}
```

The migration removes `full_name` and inserts `first_name` and `last_name`. The new fields need
no default, because the migration supplies them. See
[`examples/rename_and_split.rs`](../crates/tpt-model/examples/rename_and_split.rs).

## Merge two fields into one

The mirror image: both old fields get `removed_in`, and one migration reads both.

```rust
#[derive(Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct AddressV1 {
    #[tpt(removed_in = 2, migrate = "join_lines")]
    line1: String,
    #[tpt(removed_in = 2, migrate = "join_lines")]
    line2: String,
}
```

Use one `migrate` function for both fields, and have it remove both keys before it inserts the
merged one.

## Widen a type

`i64` or `u64` to `f64`, and an integer to a wider integer, are automatic. No recipe is needed.
Any other change of type needs a migration; see the next recipe.

## Narrow or change a type

Narrowing is never automatic, because it can lose data. The migration says how to round, clamp
or reject.

```rust
#[derive(Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct ReadingV1 {
    #[tpt(removed_in = 2, migrate = "to_centi")]
    celsius: f64,
}
```

See [`examples/custom_migration.rs`](../crates/tpt-model/examples/custom_migration.rs).

## Add an enum variant

Adding a variant is automatic: data written with the old variant names still reads, because
variant names are kept as they are.

```rust
#[derive(Serialize, Deserialize, Model)]
#[tpt(version = 2)]
enum LevelV2 {
    Low,
    Mid,
    High,
}
```

The reverse does not work. Data written with `Mid` cannot be read by a build without it. That
is what the envelope's schema id is for: the old build reports an unknown schema instead of
misreading the data. See `tests/schema_id.rs`.

## Remove or rename an enum variant

This needs a migration on the container, because the stored tree names the variant. The
migration rewrites the name.

```rust
fn sword_to_blade(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    let items = match node.remove("items") {
        Some(UirNode::Seq(items)) => items,
        _ => return Err(MigrationError::custom("items must be a sequence")),
    };
    let items = items
        .into_iter()
        .map(|item| match item {
            UirNode::Str(name) if name == "Sword" => UirNode::Str("Blade".into()),
            other => other,
        })
        .collect();
    node.insert("items", UirNode::Seq(items));
    Ok(node)
}
```

A full version is in [`examples/save_game.rs`](../crates/tpt-model/examples/save_game.rs).

**Nested enums.** If an enum is used inside a struct, the struct must be versioned as well,
because the outer version decides which enum version is read. A nested type that did not change
still resolves to its latest version that existed when the outer value was written; see
`tests/nested_versions.rs`.

## Remove a field whose data is not needed

Still a migration, but a short one. Writing it makes the removal visible in code review.

```rust
fn drop_legacy(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    node.remove("legacy_flag");
    Ok(node)
}
```

## Skip a version

Register one migration from the old version to the new one. The engine plans the shortest path,
so no intermediate version has to exist in code. If two equally short paths exist, planning
reports the ambiguity instead of guessing. See `tests/custom_migrations.rs`.

## Retire old versions

When no stored data is older than version N, run:

```sh
tpt-model squash --min-supported N --patch squash.diff crates/
```

Review the patch, then apply it with `--apply` and run `cargo check`. The tool refuses when code
that stays refers to something it would delete.

## Check a release before shipping

For each change, ask the linter. It compares two registries, typically the current code and the
previous release:

```rust
let changes = tpt_model::check::breaking_changes(&previous, &current);
print!("{}", tpt_model::check::report(&changes));
```

It reports dropped fields, added fields without a default, narrowing, removed variants, and
structural changes without a version bump.
