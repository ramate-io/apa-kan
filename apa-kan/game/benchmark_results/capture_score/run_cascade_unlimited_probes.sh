#!/usr/bin/env bash
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/capture_score"
log="$out/variant_probes_resume.log"
exec > >(tee -a "$log") 2>&1

probe() {
  local jsonl="$1" tsv="$2"
  echo "=== $jsonl -> $tsv ==="
  env -u PROBE_FANOUT_OFF PROBE_FANOUT=4 PROBE_JOBS=8 PROBE_OUT="$tsv" \
    "$out/run_deep_probe.sh" "$jsonl"
  python3 "$out/deep_probe_analyze.py" "$tsv"
}

probe "$out/smoke_cascade_mm.jsonl.gz" "$out/deep_probe_cascade_fanout.txt"
probe "$out/smoke_unlimited_mm.jsonl.gz" "$out/deep_probe_unlimited_fanout.txt"
touch "$out/variant_probes_done.flag"
echo done