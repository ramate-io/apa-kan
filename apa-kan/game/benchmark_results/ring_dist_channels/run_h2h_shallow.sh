#!/usr/bin/env bash
# Shallow H2H for 7:5:4 ring counts, ~10% braid rings, independent-channels
# on/off, and enhanced vs counterplay search. Same budget/matchups as PR #74/#78.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/ring_dist_channels"
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
)

run_one() {
  local name="$1"
  local channels="$2"
  local profile="$3"
  cargo run --release --no-default-features -p apa-kan --bin apa-kan-bench -- \
    "${common[@]}" \
    --independent-channels "$channels" \
    --search-profile "$profile" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "ring-dist ${name} shallow H2H"
}

# Usage: run_h2h_shallow.sh [a|b|a_counterplay|b_counterplay|all]
cfg="${1:-all}"
case "$cfg" in
  a) run_one h2h_a_additive_enhanced off enhanced ;;
  b) run_one h2h_b_imax_enhanced on enhanced ;;
  a_counterplay) run_one h2h_a_additive_counterplay off counterplay ;;
  b_counterplay) run_one h2h_b_imax_counterplay on counterplay ;;
  all)
    run_one h2h_a_additive_enhanced off enhanced
    run_one h2h_b_imax_enhanced on enhanced
    run_one h2h_a_additive_counterplay off counterplay
    run_one h2h_b_imax_counterplay on counterplay
    ;;
  *)
    echo "usage: $0 [a|b|a_counterplay|b_counterplay|all]" >&2
    exit 2
    ;;
esac
