# Contributing

Thanks for helping. This project is pre-1.0, and changes to the on-disk format (`SchemaId`,
envelope layout, IR encoding) need extra care. Read [docs/VERSIONING.md](docs/VERSIONING.md)
before touching them.

## Before you open a pull request

CI runs these. Run them locally first:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p tpt-model --test golden       # pinned bytes and exports
cargo build -p tpt-model --no-default-features --target thumbv7em-none-eabihf
```

The core crate is `no_std` + `alloc`. Do not add `std` imports to `crates/tpt-model/src`
outside the `#[cfg(feature = "std")]` or `global-registry` modules.

## Snapshots

* **Golden files** (`crates/tpt-model/tests/golden/`) hold byte-exact encodings and schema ids.
  If a change is meant to alter them, refresh with `TPT_UPDATE_GOLDEN=1 cargo test -p tpt-model --test golden`
  and explain the change in the pull request. An unexpected diff is a bug.
* **Compile-error snapshots** (`crates/tpt-model/tests/ui/`) are refreshed with
  `TRYBUILD=overwrite cargo test -p tpt-model --test ui`. Check the new `.stderr` by eye.

## Adding a behaviour

1. Write a test that fails without the change. Tests for evolution go in `crates/tpt-model/tests/`.
2. Keep the change small. Tick the matching item in [TODO.md](TODO.md) and add a line to
   [CHANGELOG.md](CHANGELOG.md) under *Unreleased*. Breaking changes go under *Changed (breaking)*.
3. If the change alters what `SchemaId` covers, update the list in
   [docs/VERSIONING.md](docs/VERSIONING.md).

## Dependencies

The shipped dependency graph must stay pure Rust with no `-sys` crates. `cargo deny check`
enforces licences, bans and sources. A new dependency needs a reason in the pull request.

## Commits and pull requests

Use short, imperative commit subjects. Describe in the pull request what changed and why, and
how you tested it.
