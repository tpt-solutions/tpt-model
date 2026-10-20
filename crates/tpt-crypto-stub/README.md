# tpt-crypto-stub

A local stand-in for `tpt-crypto`, the digest crate of the TPT ecosystem. It provides a `Digest`
trait, an unkeyed BLAKE3 implementation, and a keyed BLAKE3 implementation for tamper evidence.
Everything uses the pure-Rust `blake3` backend, so there is no C code in the build.

> **Stub.** The real `tpt-crypto` is not in this repository yet. The `Digest` trait is what
> [`tpt-model`](../tpt-model) depends on, so the real crate can replace these types.

`no_std`. Forbids `unsafe` code.

## What it provides

* `Digest`: `digest(&self, parts: &[&[u8]]) -> Hash32`. The result must equal the digest of the
  concatenated parts.
* `Hash32`: `[u8; 32]`.
* `Blake3`: unkeyed BLAKE3. Detects **corruption** only. Anyone who can rewrite a payload can also
  recompute its hash.
* `KeyedBlake3::new(key)`: keyed BLAKE3. Detects **corruption and forgery**, provided the key stays
  secret.
* `ct_eq(&Hash32, &Hash32) -> bool`: comparison in constant time, used for tamper checks.

## Example

```rust
use tpt_crypto_stub::{Blake3, Digest, KeyedBlake3};

let plain = Blake3.digest(&[b"payload"]);

let keyed = KeyedBlake3::new([7u8; 32]);
let tag = keyed.digest(&[b"payload"]);
assert_ne!(plain, tag);
```

Pick the keyed digest when someone other than your own code could rewrite stored data. Keep the key
out of the stored data. See [`examples/keyed_digest.rs`](../tpt-model/examples/keyed_digest.rs) in
`tpt-model`.

## Notes

* Keys are not derived or rotated by this crate. Key management belongs to the application.
* `SchemaId` in `tpt-model` is **not** computed with this crate. It identifies schemas and is not
  a security boundary.

## Minimum supported Rust version

1.90, checked in CI.

## Licence

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
