#!/usr/bin/env bash
# Shallow H2H for singles-swap on PR #79 config B
# (7:5:4, IndependentMax / independent-channels on).
#
# Locked live default: adjacent swap + one-move no-echo (`--singles-swap on`).
# Historical comparison-mode flags were removed; committed h2h_* logs stay.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/singles_swap"
mkdir -p "$out"

common=(
  --budget nodes:33000,iters:650
  --games 10
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
  --deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid
  --seed 1
  --independent-channels on
)

run_locked() {
  local name="$1"
  local profile="$2"
  cargo run --release --no-default-features -p apa-kan --bin apa-kan-bench -- \
    "${common[@]}" \
    --search-profile "$profile" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "singles-swap ${name} shallow H2H"
}

# Usage: run_h2h_shallow.sh [locked_cp]
cfg="${1:-locked_cp}"
case "$cfg" in
  locked_cp) run_locked h2h_b_locked_counterplay counterplay ;;
  *)
    echo "usage: $0 [locked_cp]" >&2
    exit 2
    ;;
esac
