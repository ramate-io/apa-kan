#!/usr/bin/env bash
# Same-deck shallow H2H at RFC ColorPass.drift diagonal values.
# Drift is the existing generation matrix (currently zeroed); --color-drift N
# sets the diagonal. Default 0 / RFC keep-last 40 / strong 160.
#
# Usage: run_h2h_color_drift.sh <name_prefix> <channels on|off> <weights> [drifts...]
#   e.g. run_h2h_color_drift.sh h2h_same_432_imax on 4,3,2 0 40 160
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/ring_dist_channels"
mkdir -p "$out"

if [[ $# -lt 3 ]]; then
  echo "usage: $0 <name_prefix> <channels on|off> <weights> [drifts...]" >&2
  exit 2
fi
prefix="$1"
channels="$2"
weights="$3"
shift 3
if [[ $# -eq 0 ]]; then
  drifts=(0 40 160)
else
  drifts=("$@")
fi

common=(
  --budget nodes:33000,iters:650
  --games 50
  --jobs 4
  --no-depth-probe
  --zags on
  --zag-mismatch sum
  --endgame-rule control-cascade
  --cascade-turn-gain on
  --capture-mode local-claim
  --no-same-piece-twice off
  --pick search
  --pairings minimax:mcts,mcts:minimax
  --same-deck on
  --search-profile counterplay
  --seed 1
)

for drift in "${drifts[@]}"; do
  name="${prefix}_drift${drift}"
  cargo run --release --offline --no-default-features -p apa-kan --bin apa-kan-bench -- \
    "${common[@]}" \
    --independent-channels "$channels" \
    --ring-count-weights "$weights" \
    --color-drift "$drift" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "same-deck ${name} ColorPass.drift ${drift} shallow H2H"
done
