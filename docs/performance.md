# Performance benchmarks

Run the dependency-free benchmark harness in an optimized build:

```sh
cargo bench --locked --bench merge
CHANGELOG_BENCH_ITERATIONS=50 CHANGELOG_BENCH_ENTRIES=10000 \
  cargo bench --locked --bench merge
```

The default workload scale is 10,000 and the default sample count is 20. Each case
warms up once, then measures the complete public library merge, including parsing,
matching, diagnostics, rendering, and clean-result validation. CSV output reports
combined input bytes, actual base entries, min/p50/p95/max wall time, and conflicts.
Percentiles use nearest rank. Input generation, the warmup, and printing are outside
the measured interval. There is no timing assertion in correctness tests.

Workloads cover independent additions and localized edits in a 10,000-entry
multiline changelog, a full release cut, 1,000 custom categories, 10,000 duplicate
entries, 10,000 reference definitions, and a 512-entry competing rewrite that
exceeds the deterministic matching budget. `scale` is the generation parameter;
`base_entries` identifies cases where references or category caps set a different
actual entry count.

## Local baseline

Measured on Linux x86_64 with Rust 1.97.1, release optimization, 20 samples per
case, and no concurrent fuzz campaign. The combined multiline inputs are about
3.14 MB. The raw samples summary is [baseline.csv](../benchmarks/baseline.csv).
These are local observations, not a guarantee for every machine or input.

| Workload | Base entries | p50 (ms) | p95 (ms) | Conflicts |
|---|---:|---:|---:|---:|
| localized_additions | 10000 | 236.447 | 256.860 | 0 |
| localized_edits | 10000 | 239.612 | 258.149 | 0 |
| release_cut | 10000 | 226.071 | 236.789 | 0 |
| many_categories | 1000 | 42.514 | 48.511 | 0 |
| duplicate_entries | 10000 | 112.971 | 121.777 | 0 |
| reference_definitions | 1 | 91.964 | 101.577 | 0 |
| rewrite_budget | 512 | 3.507 | 3.671 | 1 |

The localized 10,000-entry cases meet the one-second engineering target on this
machine. The broad rewrite intentionally returns a conflict promptly rather than
performing an unbounded similarity search.

The implementation matches exact occurrences before candidate searches, limits
similarity work deterministically, and builds bulk relocation insertion positions
once per destination. It preserves source slices and avoids serializing the full
Markdown AST. Peak-memory profiling and measurements on other platforms remain
useful follow-up work.
