#!/usr/bin/env bash
# Deep minimax fan-out on stalled capture-score JSONL positions (passivity probe).
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/capture_score"
BIN="${APA_KAN_BENCH:-target/release/apa-kan-bench}"
if [[ ! -x "$BIN" ]]; then
  cargo build --release --no-default-features -p apa-kan --bin apa-kan-bench
  BIN="target/release/apa-kan-bench"
fi

logs=()
for arg in "$@"; do
  logs+=(--probe-log "$arg")
done
if [[ ${#logs[@]} -eq 0 ]]; then
  logs=(
    --probe-log "$out/smoke_base_mm.jsonl.gz"
    --probe-log "$out/smoke_majority_mm.jsonl.gz"
  )
fi

exec "$BIN" --no-depth-probe --deep-probe \
  --budget nodes:132000,iters:2600 \
  --probe-mult 1,16,64 \
  --probe-jobs "${PROBE_JOBS:-$(nproc 2>/dev/null || echo 4)}" \
  --probe-fanout "${PROBE_FANOUT:-4}" \
  --search-profile enhanced \
  --probe-root-shuffle on \
  --probe-out "$out/deep_probe_results.txt" \
  "${logs[@]}"