# tpt-validex-stub

A local stand-in for `tpt-validex`, the validation crate of the TPT ecosystem. It checks raw
payload bytes before they are decoded, and [`tpt-model`](../tpt-model)'s load pipeline calls it
as its first content check.

> **Stub.** The real `tpt-validex` is not in this repository yet. The stub has the same shape:
> a `Validator` trait and a default `BoundsValidator`. The stub validates the byte encoding and
> size limits. It does not yet check the payload against a schema. `tpt-model` does that
> itself after decoding (`LoadError::SourceShape`).

`no_std`. Forbids `unsafe` code.

## What it provides

* `Validator`: `validate(&self, expected, bytes)`. The load pipeline calls it with an
  `ExpectedSchema` (`name` and `version`) and the raw bytes.
* `BoundsValidator`: checks that the payload is no larger than its limit and that it decodes as a
  well-formed `tpt-uir-stub` encoding.
* `ValidationError`: `TooLarge { len, max }` or `Malformed(UirError)`, with `Display`.
* `ExpectedSchema`: the schema name and version a payload is supposed to match.

## Example

```rust
use tpt_validex_stub::{BoundsValidator, ExpectedSchema, Validator};

let validator = BoundsValidator::default();
let expected = ExpectedSchema { name: "CadPart", version: 2 };

// Bytes that are not a valid encoding are rejected before any decoding into a struct.
assert!(validator.validate(expected, &[0xff, 0x00]).is_err());
```

## Relationship to the real crate

When `tpt-validex` is available, its `Validator` implementation replaces `BoundsValidator`.
`tpt-model` passes it the same `ExpectedSchema`, so the swap should not touch application code.

## Minimum supported Rust version

1.90, checked in CI.

## Licence

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
