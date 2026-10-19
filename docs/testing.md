# Testing and fuzzing

Run the ordinary checks:

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Integration tests run the actual compiled binary in temporary Git repositories.
They cover merge/rebase/cherry-pick, linked worktrees, paths with spaces, semantic
and ordinary source conflicts, and release cuts with independent additions.
CLI tests cover invalid UTF-8, input limits, invalid labels, failed output paths,
stdout separation, JSON diagnostics, Unix permissions, and symlink rejection.
Property tests check symmetry, idempotence, conservation, incompatible operations,
release membership, and arbitrary input without panics.

## Dedicated fuzz targets

The separate `fuzz` package contains:

- `parse`: arbitrary UTF-8 input, span validity, and strict no-op preservation.
- `merge`: NUL-separated base/ours/theirs input; malformed bytes become replacement
  characters. Checks determinism, clean-result parsing, and semantic symmetry.
- `structured`: generates additions/edits/deletions/moves over known base identities;
  checks conservation, incompatible-operation conflicts, and clean symmetry.

Seed inputs include nested Casita-style Markdown, concurrent additions, and
operation bytes. Generated corpora and crash artifacts belong in temporary or
ignored directories. Promote any failure into a small ordinary regression test.

With `cargo-fuzz` and a nightly toolchain available:

```sh
cargo +nightly fuzz run parse -- -max_total_time=60 -max_len=8192
cargo +nightly fuzz run merge -- -max_total_time=60 -max_len=8192
cargo +nightly fuzz run structured -- -max_total_time=60 -max_len=8192
```

For a bounded coverage-guided smoke campaign without installing cargo-fuzz:

```sh
FUZZ_SECONDS=30 scripts/fuzz-smoke.sh
```

The smoke script builds with LLVM sanitizer-coverage counters and PC tables using
an explicit Rust target, then runs all three libFuzzer targets. It requires a
compatible C++ toolchain and LLVM coverage-pass support. Each target has a
five-second per-input timeout and a 1 GiB RSS limit. Logs, corpus, and failures stay
in a printed temporary directory. This smoke mode has coverage instrumentation;
it does not enable AddressSanitizer. Use the nightly cargo-fuzz workflow for
sanitizer-backed campaigns.

Short campaigns are suitable for regression checks. Longer campaigns, retained
corpora, and independent review remain useful before production release.

## Recorded local smoke result

On Linux x86_64 with Rust 1.97.1, the final coverage-only smoke rerun used seed
12345, an 8,192-byte maximum input, and a ten-second budget per target. All targets
exited successfully:

| Target | Executions | Observed failures |
|---|---:|---:|
| parse | 8,512 | 0 |
| merge | 2,201 | 0 |
| structured | 2,250 | 0 |

Earlier thirty-second campaigns also completed without failures. Execution counts
vary with machine load and corpus evolution. These short runs do not replace
longer fuzzing or sanitizer-backed validation.
