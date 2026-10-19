# Changelog

All notable changes to this project are documented here.

The format follows Keep a Changelog 1.1.

## [Unreleased]

## [0.1.0] - 2026-10-19

### Added

- Cross-platform GitHub Actions CI for Linux, macOS, Windows, and Rust 1.88.0.
- Source-preserving Markdown parser and conservative semantic three-way merge library.
- Git merge-driver CLI with diff3 conflicts, atomic output, and JSON diagnostics.
- Parser, merge, property, CLI, and real Git integration tests.
- Local Nix package definition.

### Fixed

- Preserve ordering constraints across overlapping entry moves on both branches.
- Keep source fixtures consistent across Windows, macOS, and Linux checkouts.

### Changed

- Reconcile unambiguous release cuts while retaining independent post-cut work.
- Match larger rewrite gaps, section renames, edited moves, and compatible reorders.
- Preserve interleaved opaque Markdown and merge reference definitions independently.
- Retain one-sided whitespace changes and untouched mixed line endings.
- Report structured merge events and validate entry membership and reference inventories.
- Add coverage-guided fuzz targets, failure-path tests, bounded matching, and benchmarks.

[Unreleased]: https://github.com/cachix/changelog-merge/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/cachix/changelog-merge/releases/tag/v0.1.0
