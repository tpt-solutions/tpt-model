# tpt-uir-stub

A local stand-in for `tpt-uir`, the intermediate-representation (IR) crate of the TPT ecosystem.
It provides the tree type that [`tpt-model`](../tpt-model) migrates, and a strict, bounds-checked
byte encoding for it.

> **Stub.** The real `tpt-uir` is not in this repository yet. The encoding here only has to be
> deterministic and safe on untrusted input. Its public names are chosen so the real crate can
> replace it.

`no_std` with `alloc`. Forbids `unsafe` code.

## What it provides

* `UirNode`: the tree. Variants `Null`, `Bool`, `Int(i64)`, `UInt(u64)`, `Float(f64)`, `Str`,
  `Bytes`, `Seq` and `Map`. Maps keep insertion order.
* Field access on maps: `get`, `insert`, `remove`.
* `UirNode::canonicalize`: an order-independent form, for when map order must not matter.
* `encode(&UirNode) -> Vec<u8>` and `decode(&[u8]) -> Result<UirNode, UirError>`.
* Limits: `MAX_DEPTH` (64) and `MAX_LEN` (16 MiB) for strings, byte strings, sequences and maps.

## Example

```rust
use tpt_uir_stub::{decode, encode, UirNode};

let mut node = UirNode::Map(vec![]);
node.insert("name", UirNode::Str("bracket".into()));
node.insert("weight", UirNode::Float(2.0));

let bytes = encode(&node);
assert_eq!(decode(&bytes).unwrap(), node);
```

## Untrusted input

`decode` never panics on garbage. It returns `UirError` for:

* truncated or trailing bytes;
* nesting deeper than `MAX_DEPTH`;
* lengths above `MAX_LEN`.

Preallocation for sequences and maps is capped at 1024 elements. Larger collections still decode,
but their storage grows as the elements arrive, so a short input cannot reserve much memory.

## Relationship to the real crate

When `tpt-uir` is available, its tree and encoding replace this crate. `tpt-model` depends on the
names above, so the swap should be a dependency change plus any renamed items.

## Minimum supported Rust version

1.90, checked in CI.

## Licence

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
