# Error FAQ

Each entry gives the error as it is printed, what causes it, and what to do. Messages come from
the code in `crates/tpt-model/src`.

## Compile-time (the derive)

**`#[derive(Model)] does not support generic types`**
Generic structs have no single schema. Write one concrete struct per version.

**`128-bit integers are not supported by tpt-model`**
`u128` and `i128` have no IR representation. Use `u64` and `i64`, or a byte array in a `Vec<u8>`.

**`fixed-size arrays are not supported`**
Use `Vec<T>`. The length of the array is not part of the schema.

**`tuples are not supported`**
Use a named struct with named fields.

**`field ... removed_in requires migrate`**
A removed field needs a migration to say where its data goes, even if the data is discarded.
Add `migrate = "function_name"` and write the function. See the
[cookbook](COOKBOOK.md#remove-a-field-whose-data-is-not-needed).

## Registration

**`X v2 is not registered`** (`NotRegistered`)
A load or plan refers to a version that was never registered. Call `registry.register::<T>()` for
that version, or register the migration that includes it.

**`duplicate migration vN -> vM`** (`DuplicateMigration`)
Two `impl Migration` items cover the same pair of versions. Keep one.

**`registry: conflicting schemas registered for X vN`** (`VersionConflict`)
Two different structs claim the same name and version, usually after copying a struct without
bumping its version. Give one of them the next version number.

**`cycle between named types`** (`Cycle`)
Two types contain each other, directly or through other types. The schema graph must be acyclic,
and `Box` does not change that because it is erased. Store an id in one of the types instead of
the value, and look the value up when you need it.

## Planning

**`X: no migration path from vA to vB`** (`NoPath`)
No registered migration or consecutive version links the two. Register a migration from vA to
vB, or register the missing intermediate version.

**`X: version vN is missing from the chain`** (`Gap`)
A version between the stored one and the target is not registered, and no migration jumps over
it. Register the missing version, or write a migration that goes straight from the stored version
to the target.

**`X: N equally short paths from vA to vB`** (`Ambiguous`)
Two routes have the same length, so the engine will not choose. Remove one of the migrations, or
register a direct one from vA to vB.

**`X: data at vN is older than the minimum supported version vM`** (`BelowMinSupported`)
The data is older than the oldest version the application still reads. This is the signal that
the data must be re-saved by an earlier release, or that `min_supported` was raised too early.

**`X: data at vN is newer than target vM`** (`Downgrade`)
The data was written by a newer release. Upgrade the application; the engine does not read
forward.

## Loading

**`envelope hash mismatch: data was modified`** (`Tampered`)
The stored bytes changed after they were written, or they were sealed with a different key.
Restore from a backup. If you use the unkeyed digest, this only detects corruption, not a
deliberate edit; see the [keyed digest example](../crates/tpt-model/examples/keyed_digest.rs).

**`not a TPT envelope (bad magic)`** (`BadMagic`), **`envelope truncated: need N bytes, have M`**,
**`N unexpected trailing bytes`**, **`unsupported envelope format N`**
The file is not an envelope, or it was cut short, appended to, or written by a newer format.
Check the source of the file.

**`X v1: stored data does not match its schema: ...`** (`SourceShape`)
The stored tree is not valid for the version it claims. The most common cause is a field added
to the struct after the data was written, without a version bump. The error names the path, for
example `$.limits.width`. Bump the version, or restore the data.

**`X: field `f` is not in vN (from vM)`** (`UnknownField`)
The stored data has a field the target does not know, and no rule removes it. Add
`removed_in` and a migration to the field's struct, or rename the field with `rename`.

**`X: field `f` added in vN has no default (from vM)`** (`MissingField`)
A new field has no default and is not optional. Add `default = "..."`, make the field `Option`, or
write a migration that supplies it.

**`X: field `f` changes type in vN ... an explicit migration is required`** (`TypeChange`)
The type changed in a way that is not widening. Write a migration that converts the value.

**`X: result of vA -> vB is invalid: ...`** (`InvalidResult`)
A migration produced a tree that does not match the target version. The message names the path.
Usually the migration forgot to remove a key or to insert a required one.

**`hydration failed: ...`**
The tree matched the schema but could not be turned into the Rust struct. This usually means a
field type in the struct differs from the schema, for example a `Vec` where the tree holds a
map.

## Defaults

**`invalid default `...`: ...`**
The `default` expression is not in the supported grammar. Literals (`42`, `-1.5`, `1e5`,
`true`, `None`), strings in double quotes (escapes `\"`, `\\`, `\n`, `\r`, `\t`), `[]`, `{}`, and
`Type::Variant` are accepted. Anything else is rejected.

## CLI

**`refusing: minimum supported vN is newer than the latest version vM of X`**
You asked to keep a version that does not exist. Check `--min-supported` against the highest
`version` in the source.

**`refusing: X would be deleted but is still used by ... at file:line`**
Code that stays refers to a version the squash would delete. Update that code first, or raise
`--min-supported`.

**`no versioned model named X in the scanned sources`** (`new-migration`)
The name passed to `--name` does not match a `#[derive(Model)]` type in the paths given. Names
are the logical names (`name = "..."`), not Rust struct names.

**`malformed #[tpt] attribute`**
An attribute did not parse. The scanner stops rather than guessing, so fix the attribute first.

**`refusing: removing `X` deletes whole lines, and Y shares line N`** (`SharesLines`)
Removal deletes whole lines. A kept item on one of those lines would be deleted too. Move the
kept item onto its own line, then run the squash again.

**`squash` refuses for a type it should not**
The tool matches names, not module paths, so a same-named type in another module can cause a
refusal. This errs toward refusing: it never removes a type that code still uses under the same
name. Rename one of the types, or squash the module that owns the old one on its own.
