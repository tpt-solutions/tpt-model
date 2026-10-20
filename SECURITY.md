# Security policy

## Reporting a vulnerability

Please report security problems privately, through GitHub's private vulnerability reporting on
this repository (Security tab, then *Report a vulnerability*). Do not open a public issue for a
vulnerability.

Include the affected crate and version, a description of the impact, and a way to reproduce it.
Expect an acknowledgement within a few days. We will agree a disclosure date with you before
anything is published.

## Supported versions

The project is pre-1.0. Only the latest commit on `master` receives fixes.

## Scope and known limits

* **Unkeyed `Blake3` detects corruption only.** Anyone who can rewrite stored bytes can also
  recompute the hash. Use `KeyedBlake3` (in `tpt-crypto-stub`) when data must be protected
  against deliberate modification, and keep the key secret.
* **Field access rules** (`intent`, `capability`) are enforced only where the caller applies
  them, through `tpt_model::access`. The engine does not check on every read.
* **The `tpt-*-stub` crates are stand-ins** for TPT crates that do not exist yet. Their
  validation and cryptography are not the production implementations.
* **The `squash` CLI** rewrites source files when run with `--apply`. Review the plan (or the
  `--patch` output) first, and keep the work in version control.
