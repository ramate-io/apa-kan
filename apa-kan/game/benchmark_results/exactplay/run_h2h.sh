#!/usr/bin/env bash
# Shallow exactplay vs counterplay package on live RuleSet defaults.
#
# Usage:
#   run_h2h.sh all          # self-play + both H2H seats
#   run_h2h.sh self         # exactplay vs exactplay
#   run_h2h.sh h2h_p1       # P1 exactplay vs P2 counterplay
#   run_h2h.sh h2h_p2       # P1 counterplay vs P2 exactplay
#   run_h2h.sh counter      # counterplay vs counterplay (old-bot baseline)
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/exactplay"
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
  --seed 1
)

run_one() {
  local name="$1"
  shift
  cargo run --release --offline --no-default-features -p apa-kan --bin apa-kan-bench -- \
    "${common[@]}" \
    "$@" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "exactplay ${name} shallow H2H"
}

cfg="${1:-all}"
case "$cfg" in
  self)
    run_one h2h_self --search-profile exactplay
    ;;
  h2h_p1)
    run_one h2h_exact_as_p1 --p1-search-profile exactplay --p2-search-profile counterplay
    ;;
  h2h_p2)
    run_one h2h_exact_as_p2 --p1-search-profile counterplay --p2-search-profile exactplay
    ;;
  counter)
    run_one h2h_counter --search-profile counterplay
    ;;
  all)
    run_one h2h_self --search-profile exactplay
    run_one h2h_exact_as_p1 --p1-search-profile exactplay --p2-search-profile counterplay
    run_one h2h_exact_as_p2 --p1-search-profile counterplay --p2-search-profile exactplay
    ;;
  *)
    echo "usage: $0 [all|self|h2h_p1|h2h_p2|counter]" >&2
    exit 2
    ;;
esac
