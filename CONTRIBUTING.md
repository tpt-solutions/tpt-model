# Contributing

Contributions are made through GitHub issues only. Pull requests are not accepted. Open an
issue to report a bug, ask a question, or propose a change, and the maintainers will decide
how it is made.

This project is pre-1.0. Changes to the on-disk format (`SchemaId`, envelope layout, IR
encoding) need extra care. Read [docs/VERSIONING.md](docs/VERSIONING.md) before raising one.

## Reporting a bug

Include:

* The `tpt-model` version or commit, and your Rust version (`rustc --version`).
* Your operating system and target (for example `x86_64-pc-windows-msvc`).
* A minimal reproduction: the type definitions, the code that saves or loads, and the error or
  unexpected output.
* For a stored-data problem, the output of `tpt-model inspect` on the entity, if you can share it.

Do not attach private or sensitive data. For security issues, follow [SECURITY.md](SECURITY.md)
instead of opening a public issue.

## Proposing a change

Describe the problem first, then the change you would like. For changes to the on-disk format,
say which golden files or `SchemaId` values would change, and why that is acceptable.

## Licence

By opening an issue you agree that any code or text you post may be used under the project
licence, [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at the maintainers' option.
