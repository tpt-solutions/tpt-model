# Changelog

All notable changes to `tpt-uir-stub` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace-wide log is in the
[root changelog](../../CHANGELOG.md).

## [Unreleased]

### Added

* `UirNode` tree with `get`, `insert`, `remove` and `canonicalize`.
* Deterministic `encode` and bounds-checked `decode`, with `UirError` for malformed input.
* `MAX_DEPTH` and `MAX_LEN` limits.
* Crate README and keywords and categories.

### Fixed

* The decoder no longer reserves memory from an unchecked length prefix. Preallocation is capped at
  1024 elements; larger collections still round-trip.

## [0.1.0]

Not released yet. The first release will be tagged after the v1.0.0 decision.
