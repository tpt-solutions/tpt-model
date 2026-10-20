# tpt-primitives-stub

A local stand-in for `tpt-primitives`, the shared access-control primitives of the TPT ecosystem.
It models what a field-level access check needs: an `Intent` (what the caller wants to do) and a
`Capability` (a named grant the caller holds).

> **Stub.** The real `tpt-primitives` is not in this repository yet. Field access rules in
> [`tpt-model`](../tpt-model) (`#[tpt(intent = "...", capability = "...")]`) are written against these
> types, so the real crate can replace them.

`no_std`. Forbids `unsafe` code.

## What it provides

* `Intent`: `Read` or `Write`. `Intent::parse` accepts `"read"` and `"write"` (and their
  capitalized forms), as used in attributes. `Display` prints the lowercase spelling.
* `Capability(pub &'static str)`: a named grant, such as `"cad.edit"`.
* `is_granted(required: Option<Capability>, granted: &[Capability]) -> bool`: `true` when no
  capability is required, or when the required one is in the granted list.

## Example

```rust
use tpt_primitives_stub::{is_granted, Capability, Intent};

assert_eq!(Intent::parse("write"), Some(Intent::Write));

let edit = Capability("cad.edit");
assert!(is_granted(Some(edit), &[edit]));
assert!(!is_granted(Some(edit), &[]));
assert!(is_granted(None, &[]));
```

## Notes

* This crate only answers "is this grant present?". It does not decide who holds a grant. That is
  the caller's responsibility.
* `tpt-model` runs an access check only when the caller asks for one (`tpt_model::access::check`).

## Minimum supported Rust version

1.90, checked in CI.

## Licence

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
