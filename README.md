# changelog-merge

Semantic three-way merging for Keep a Changelog, written in Rust. Contributors
continue editing `CHANGELOG.md` directly. Independent list entries merge together;
incompatible edits produce ordinary Git conflict markers with base context.

Initial version: 0.1.0. Licensed under Apache-2.0.

```markdown
## [Unreleased]

### Added

- Added S3 support.
- Added SSH support.
- Initial feature.
```

When separate branches add the S3 and SSH entries above, both survive. Concurrent
additions retain each branch's order. Available entries are selected by canonical
source bytes to break ordering ties, independent of which branch is ours.

## Install and use

Requires Rust 1.88 or newer.

```sh
cargo install changelog-merge --version 0.1.0 --locked
changelog-merge check CHANGELOG.md
changelog-merge merge base.md ours.md theirs.md
changelog-merge merge base.md ours.md theirs.md --output merged.md
changelog-merge merge base.md ours.md theirs.md --output - --diagnostics json
```

To build from this repository, use `cargo build --release --locked` or
`cargo install --path . --locked`.

The default output is `ours.md`. `--output -` writes Markdown to stdout and leaves
the inputs alone. Diagnostics use stderr. `--explain` reports per-entry changes,
renames, moves, formatting selection, ordering, deduplication, and conflicts.
`--diagnostics json` includes structured events and source-local spans.
`--strict` rejects inferred entry matches during concurrent merges. Invalid UTF-8,
files above 32 MiB, and documents beyond the documented complexity budgets are
rejected without replacing file output.

Exit status: `0` clean, `1` unresolved conflicts, `2` input/validation/I/O error,
`129` caught internal panic. Errors leave file output unchanged. Help and usage
are available with `--help` on either subcommand.

## Git integration

Track this in `.gitattributes` (already present in this repository):

```gitattributes
CHANGELOG.md merge=changelog-merge
```

Supply the following runtime configuration, either in Git configuration or through
Git's environment variables:

```ini
[merge "changelog-merge"]
    name = Keep a Changelog semantic merger
    driver = changelog-merge merge %O %A %B --marker-size %L --path %P
    recursive = binary
```

Attributes alone do not configure or install the driver. Git can fall back to text
merging when the driver is undefined. Every contributor and CI environment needs
the executable and runtime configuration for semantic behavior. The CLI never
runs Git or changes Git configuration. See [Git integration](docs/git-integration.md)
for a configuration that leaves `.git/config` and global settings untouched.

## Semantics and limits

Multiline entries, nested lists, code fences, Unicode, custom categories, and
reference links retain their original Markdown source. The engine merges releases,
categories, and entries relative to the common ancestor. Exact matching preserves
base duplicate occurrences by position. Larger rewrite gaps use conservative,
unique candidate matching. One-sided renames, compatible reorders, and unambiguous
moves merge without rewriting their entry bodies.

Release cuts keep released entries in the new version and independent new work
in `Unreleased`. Opaque paragraphs/comments between entries are retained, and
independent opaque lines and reference definitions merge. Competing edits or
uncertain identity/membership remain visible conflicts.

Read [merge semantics](docs/merge-semantics.md) and
[limitations](docs/limitations.md) before adoption. [CI](https://github.com/cachix/changelog-merge/actions/workflows/ci.yml)
tests Linux, macOS, Windows, and the minimum supported Rust version. Independent
security review remains part of production hardening.

## Nix

`nix-build` builds the local `default.nix` package, using `Cargo.lock` and running
the tests with Git available. Supply a pinned `pkgs` argument for reproducible
nixpkgs selection with Rust 1.88 or newer. No devenv configuration is included yet.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Tests invoke the compiled executable inside temporary repositories for merge,
rebase, cherry-pick, linked worktrees, paths with spaces, and ordinary source-file
conflicts. No network service or telemetry is used by the executable.

Dedicated fuzz targets and a bounded coverage-guided smoke script are documented
in [testing](docs/testing.md). Run repeatable release-build benchmarks with:

```sh
cargo bench --locked --bench merge
```

See [performance](docs/performance.md) for workloads and measured local results.
