#!/usr/bin/env bash
# Shallow H2H for singles-swap modes on PR #79 config B
# (7:5:4, IndependentMax / independent-channels on).
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

run_one() {
  local name="$1"
  local swap="$2"
  local profile="$3"
  cargo run --release --no-default-features -p apa-kan --bin apa-kan-bench -- \
    "${common[@]}" \
    --singles-swap "$swap" \
    --search-profile "$profile" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "singles-swap ${name} shallow H2H"
}

# Usage: run_h2h_shallow.sh [adj_any_cp|adj_own_cp|any_cp|own_cp|adj_any_en|adj_own_en|any_en|own_en|all]
cfg="${1:-all}"
case "$cfg" in
  adj_any_cp) run_one h2h_b_adj_any_counterplay adjacent-any counterplay ;;
  adj_own_cp) run_one h2h_b_adj_own_counterplay adjacent-own counterplay ;;
  any_cp) run_one h2h_b_any_counterplay any counterplay ;;
  own_cp) run_one h2h_b_own_counterplay own counterplay ;;
  adj_any_en) run_one h2h_b_adj_any_enhanced adjacent-any enhanced ;;
  adj_own_en) run_one h2h_b_adj_own_enhanced adjacent-own enhanced ;;
  any_en) run_one h2h_b_any_enhanced any enhanced ;;
  own_en) run_one h2h_b_own_enhanced own enhanced ;;
  all)
    run_one h2h_b_adj_any_counterplay adjacent-any counterplay
    run_one h2h_b_adj_own_counterplay adjacent-own counterplay
    run_one h2h_b_any_counterplay any counterplay
    run_one h2h_b_own_counterplay own counterplay
    run_one h2h_b_adj_any_enhanced adjacent-any enhanced
    run_one h2h_b_adj_own_enhanced adjacent-own enhanced
    run_one h2h_b_any_enhanced any enhanced
    run_one h2h_b_own_enhanced own enhanced
    ;;
  *)
    echo "usage: $0 [adj_any_cp|adj_own_cp|any_cp|own_cp|adj_any_en|adj_own_en|any_en|own_en|all]" >&2
    exit 2
    ;;
esac
