#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
fuzz_host=$(rustc -vV | sed -n 's/^host: //p')
export CARGO_TARGET_DIR="$PWD/target/fuzz-instrumented"
export RUSTFLAGS='-C passes=sancov-module -C llvm-args=-sanitizer-coverage-level=3 -C llvm-args=-sanitizer-coverage-inline-8bit-counters -C llvm-args=-sanitizer-coverage-pc-table'
cargo build --locked --manifest-path fuzz/Cargo.toml --target "$fuzz_host" --bins
fuzz_campaign=$(mktemp -d "${TMPDIR:-/tmp}/changelog-merge-fuzz.XXXXXX")
mkdir -p "$fuzz_campaign/artifacts"
for fuzz_target in parse merge structured; do
  mkdir -p "$fuzz_campaign/$fuzz_target"
  cp fuzz/seeds/* "$fuzz_campaign/$fuzz_target/"
  if "$CARGO_TARGET_DIR/$fuzz_host/debug/$fuzz_target" "$fuzz_campaign/$fuzz_target" \
    -seed=12345 -max_total_time="${FUZZ_SECONDS:-30}" -max_len=8192 \
    -timeout=5 -rss_limit_mb=1024 -artifact_prefix="$fuzz_campaign/artifacts/" \
    > "$fuzz_campaign/$fuzz_target.log" 2>&1; then
    tail -n 1 "$fuzz_campaign/$fuzz_target.log"
  else
    cat "$fuzz_campaign/$fuzz_target.log"
    echo "Fuzz failure, logs and artifacts: $fuzz_campaign" >&2
    exit 1
  fi
done
echo "Fuzz logs and corpus: $fuzz_campaign"
