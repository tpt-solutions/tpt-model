# {{project-name}}

A versioned data model generated from the tpt-model template.

## First steps

```sh
TPT_UPDATE_SNAPSHOTS=1 cargo test    # writes tests/snapshots/schema.json; review it and commit it
cargo test                           # checks the code against the snapshot and the evolutions
```

## Changing the model

1. Add the next version as a new struct with `#[tpt(version = N)]`.
2. Mark what the old version loses with `removed_in` and a `migrate` function.
3. Add a test in `tests/evolution.rs`, using `assert_evolves!` or `Loader::load`.
4. Regenerate the snapshot with `TPT_UPDATE_SNAPSHOTS=1 cargo test` and review the diff.

The [cookbook](https://github.com/tpt-solutions/tpt-model/blob/master/docs/COOKBOOK.md) covers
the common changes, and the [error FAQ](https://github.com/tpt-solutions/tpt-model/blob/master/docs/FAQ.md)
explains the messages you may see.
