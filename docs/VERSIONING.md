# Versioning rules

These rules keep persisted data readable across releases. The compiler, the derive
macro and the load pipeline enforce most of them.

## One struct per version

Each version of a type is its own Rust struct, and all versions share a logical name:

```rust
#[derive(Model, Serialize, Deserialize)]
#[tpt(version = 1)]
struct CadPartV1 { /* ... */ }

#[derive(Model, Serialize, Deserialize)]
#[tpt(version = 2)]
struct CadPartV2 { /* ... */ }
```

The logical name defaults to the type name without a trailing `V<digits>`, so both
structs are `CadPart`. Set it explicitly with `#[tpt(name = "CadPart")]` when the
type name does not follow that pattern.

Old structs are kept until the squash tool removes them (see below). The current
struct is the only one used in application code.

## Version numbers

* `version` starts at 1 and increases by one for each release that changes the shape.
  Gaps are allowed, but the load pipeline reports a missing version as a `Gap`.
* Every version that data may have been written with must be registered.
* Version numbers never change once released.

## Field attributes

| Attribute | Meaning | Rule enforced at compile time |
|-----------|---------|-------------------------------|
| `since = N` | Added in version N. Defaults to 1. | `1 <= since <= version` |
| `removed_in = M` | Present in this version, removed in version M. | `M > version` and `M > since` |
| `migrate = "fn"` | Function that handles the removed data. | Required with `removed_in`; must exist with the migration signature |
| `default = "expr"` | Value used when the field is added (see below). | Checked at load time |
| `rename = "old"` | Former name. Repeatable. | Checked at load time |
| `intent`, `capability` | Field access rule (see `access`). | `intent` is `"read"` or `"write"` |
| `min = N`, `max = N` | Value bounds. Lengths for strings, bytes and sequences. | `min <= max` |

A field removed in a later version must say where its data goes. Without `migrate` the
derive refuses to compile, so data is never dropped silently.

## Adding a field

Give the new field a `default`, or make it an `Option`. Otherwise an old entity cannot be
loaded as the new version.

```rust
#[tpt(since = 2, default = "Material::Titanium")]
material: Material,
```

## Renaming a field

Keep the old name in the *new* struct's `rename` list:

```rust
#[tpt(rename = "name")]
title: String,
```

## Changing a type

* **Widening** is automatic. Integers are accepted by any integer type that can hold them, and
  integers and floats are accepted by `f64`.
* **Narrowing or any other change** needs an explicit migration. The automatic step
  reports `TypeChange` instead of guessing.

## Removing a field

Put `removed_in` and `migrate` on the *old* struct's field, then implement a migration
from the old version to the new one. See the [migration author guide](MIGRATION_GUIDE.md).

## Minimum supported version

Set `Loader::min_supported` to refuse data older than a floor:

```rust
loader.min_supported = Some(8); // "This build opens files back to v8."
```

Data below the floor fails with `BelowMinSupported`, and no migration runs.

## Retiring old versions

Old versions become dead weight. Once a release no longer needs to read them, run the
squash tool:

```text
tpt-model squash --min-supported 8 --patch squash.diff src/
```

The tool prints a plan. It refuses when code that stays still refers to a removed type.
`--apply` edits the files, and `--patch` writes the same change as a diff.

## Changing a schema without a version bump

A change to a struct's shape changes its `SchemaId`. If the old shape is no longer registered,
data written under it fails with `UnknownSchema` instead of being misread. Bump the version
whenever the shape changes. The golden tests in `crates/tpt-model/tests/golden.rs` pin
the ids, so an accidental change fails the build.
