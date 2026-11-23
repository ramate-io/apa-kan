#!/usr/bin/env bash
# Fan-out deep probes + unlimited smoke for capture-score DEEP_PROBE.md
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/capture_score"
BIN="${APA_KAN_BENCH:-target/release/apa-kan-bench}"
budget="nodes:132000,iters:2600"
log="$out/variant_probes_batch.log"
exec > >(tee -a "$log") 2>&1

probe_one() {
  local jsonl="$1" tsv="$2"
  echo "=== deep-probe $jsonl -> $tsv (fan-out 4) ==="
  PROBE_FANOUT=4 PROBE_JOBS="${PROBE_JOBS:-8}" PROBE_OUT="$tsv" \
    "$out/run_deep_probe.sh" "$jsonl"
  python3 "$out/deep_probe_analyze.py" "$tsv" || true
}

if [[ ! -f "$out/smoke_unlimited_mm.jsonl.gz" ]]; then
  echo "=== smoke unlimited (4 games seed 904) ==="
  "$BIN" --no-depth-probe --games 4 --seed 904 --jobs 2 \
    --budget "$budget" \
    --bots minimax --pairings minimax:minimax \
    --objective capture-score --unlimited-spawn on \
    --search eval_capture=0 \
    --out "$out/smoke_unlimited_mm.txt"
  python3 "$out/qualitative_read.py" "$out/smoke_unlimited_mm.jsonl.gz" || true
fi

probe_one "$out/smoke_majority_mm.jsonl.gz" "$out/deep_probe_majority_fanout.txt"
probe_one "$out/smoke_cascade_mm.jsonl.gz" "$out/deep_probe_cascade_fanout.txt"
probe_one "$out/smoke_unlimited_mm.jsonl.gz" "$out/deep_probe_unlimited_fanout.txt"

echo "=== variant probe batch done ==="
touch "$out/variant_probes_done.flag"