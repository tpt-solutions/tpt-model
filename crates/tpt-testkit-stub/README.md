# tpt-testkit-stub

A local stand-in for `tpt-testkit`, the test helper crate of the TPT ecosystem. It is for tests
only and is not published (`publish = false`).

> **Stub.** The real `tpt-testkit` is not in this repository yet. Only the helpers that the
> workspace tests use are here.

`std` (test code only).

## What it provides

* `assert_golden(path, actual)`: compares `actual` with the golden file at `path`. Line endings are
  normalized, so CRLF and LF match. Set `TPT_UPDATE_GOLDEN=1` to write the file instead of comparing.
  A missing golden file panics with the command that creates it.
* `hex(&[u8]) -> String`: lowercase hexadecimal, for readable golden files.

## Example

```rust
use tpt_testkit_stub::{assert_golden, hex};

let bytes = [0xde, 0xad, 0xbe, 0xef];
assert_golden("tests/golden/envelope.hex", &hex(&bytes));
```

Create or refresh the file with:

```sh
TPT_UPDATE_GOLDEN=1 cargo test
```

Review the diff before committing it. Golden files pin exact bytes and schema ids, so a change in
them is a change in the on-disk format.

## Minimum supported Rust version

1.90, checked in CI.

## Licence

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
