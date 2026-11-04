# Changelog

All notable changes to `tpt-crypto-stub` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace-wide log is in the
[root changelog](../../CHANGELOG.md).

## [Unreleased]

### Changed (breaking, before 1.0)

* `Blake3` is replaced by `Sha256` (pure-Rust `sha2`). `KeyedBlake3` is replaced by `KeyedSha256`
  (HMAC-SHA256 via `hmac`).
* `Digest` gains `fn id(&self) -> u8`, naming the algorithm. Constants are in `tpt_crypto_stub::id`.

### Added

* `Digest` trait, `Hash32` and the unkeyed `Blake3` digest (pure-Rust backend).
* `KeyedBlake3` for tamper evidence against forgery.
* `ct_eq` for constant-time comparison of digests.
* Crate README and keywords and categories.

## [0.1.0]

Not released yet. The first release will be tagged after the v1.0.0 decision.
