# tpt-model-derive

The `#[derive(Model)]` procedural macro for [`tpt-model`](../tpt-model). It describes each
version of a struct or enum as `const` schema data, and checks the evolution rules at compile time.

You do not normally depend on this crate directly. `tpt-model` re-exports the derive as
`tpt_model::Model`.

```rust
use serde::{Deserialize, Serialize};
use tpt_model::{EntityId, Model};

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2)]
struct CadPartV2 {
    id: EntityId,
    name: String,
    #[tpt(since = 2, default = "0")]
    revision: u32,
    #[tpt(rename = "label")]
    title: String,
}
```

## Attributes

### Type level (`struct` or `enum`)

| Attribute | Default | Meaning |
|-----------|---------|---------|
| `version = N` | `1` | Schema version of this struct. |
| `name = "..."` | type name without a trailing `V<N>` | Logical name shared by all versions. |

### Field level

| Attribute | Meaning |
|-----------|---------|
| `since = N` | Version the field was added in (default 1). |
| `removed_in = N` | Version the field is removed in. Requires `migrate`. |
| `migrate = "path::to::fn"` | Migration function for the removed data. Its signature is checked. |
| `default = "expr"` | Default used by automatic evolution. See `tpt_model::default_expr`. |
| `rename = "old"` | Former name. May be repeated. |
| `intent = "read" \| "write"` | Access intent for `tpt_model::access`. |
| `capability = "..."` | Capability a caller needs for the field. |
| `min = N`, `max = N` | Value bounds, or length bounds for strings, bytes and sequences. |

### Variant level (enums: unit and newtype variants)

`since`, `removed_in` and `migrate`, with the same meaning as on fields.

## Compile-time errors

The macro rejects, with a span on the offending attribute:

* a removed field or variant without `migrate`;
* version ranges that do not make sense (for example `removed_in` before `since`);
* unknown attributes and keys;
* migration functions with the wrong signature;
* `u128`, `i128`, arrays and tuples, which the schema model does not describe yet.

Type aliases are not seen through. They become named references, and
`Registry::check_graph` catches them at runtime.

Failure cases are tested with `trybuild` in `tests/ui/`.

## Minimum supported Rust version

1.90, checked in CI.

## Licence

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
