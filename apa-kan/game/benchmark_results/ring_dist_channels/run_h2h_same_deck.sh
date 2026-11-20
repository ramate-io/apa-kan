#!/usr/bin/env bash
# Same-deck shallow H2H: both players start with the identical 27-tile
# collection. Matrix: {4:3:2, 4:3:1} × {independent-max, additive} plus a
# 7:5:4 independent-max baseline. Counterplay search. 50 games × 2 pairings
# × mixed:mixed = 100 games per config.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/ring_dist_channels"
mkdir -p "$out"

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

run_one() {
  local name="$1"
  local channels="$2"
  local weights="$3"
  cargo run --release --no-default-features -p apa-kan --bin apa-kan-bench -- \
    "${common[@]}" \
    --independent-channels "$channels" \
    --ring-count-weights "$weights" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "same-deck ${name} shallow H2H"
}

# Usage: run_h2h_same_deck.sh [432_imax|432_add|431_imax|431_add|754_imax|all]
cfg="${1:-all}"
case "$cfg" in
  432_imax) run_one h2h_same_432_imax on 4,3,2 ;;
  432_add) run_one h2h_same_432_add off 4,3,2 ;;
  431_imax) run_one h2h_same_431_imax on 4,3,1 ;;
  431_add) run_one h2h_same_431_add off 4,3,1 ;;
  754_imax) run_one h2h_same_754_imax on 7,5,4 ;;
  all)
    run_one h2h_same_432_imax on 4,3,2
    run_one h2h_same_432_add off 4,3,2
    run_one h2h_same_431_imax on 4,3,1
    run_one h2h_same_431_add off 4,3,1
    run_one h2h_same_754_imax on 7,5,4
    ;;
  *)
    echo "usage: $0 [432_imax|432_add|431_imax|431_add|754_imax|all]" >&2
    exit 2
    ;;
esac
