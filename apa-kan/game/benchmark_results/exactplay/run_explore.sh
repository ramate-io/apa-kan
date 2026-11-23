#!/usr/bin/env bash
# Deep randomized / wider search vs standard exactplay at the same 4x (and
# optional 8x) in-game budget. This is NOT a weak/random-player baseline:
# heuristic playouts stay on; the fan-out is higher PUCT, Dirichlet root
# noise, shuffled / unordered moves, and LMR/PVS off.
#
# Committed explore/ logs predate PR #88 (no ratchet, 1-1-1-1). Re-runs
# inherit live RuleSet defaults (ratchet + 2-2-1-1) unless those flags are
# pinned off.
#
# Usage:
#   run_explore.sh all
#   run_explore.sh wide4_p1 | wide4_p2 | wide4_p1_s11 | wide4_p2_s11
#   run_explore.sh wide8_p1 | wide8_p2
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/exactplay/explore"
mkdir -p "$out"

BIN="${APA_KAN_BENCH:-target/release/apa-kan-bench}"
if [[ ! -x "$BIN" ]]; then
  cargo build --release --no-default-features -p apa-kan --bin apa-kan-bench
  BIN="target/release/apa-kan-bench"
fi

LIGHT="nodes:33000,iters:650"
DEEP4="nodes:132000,iters:2600"
DEEP8="nodes:264000,iters:5200"
WIDE="puct=8,root-noise=0.25,dirichlet-alpha=0.3,ordering=off,lmr=off,pvs=off,untried-random=1"

common=(
  --budget "$LIGHT"
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
  --search-profile exactplay
)

run_one() {
  local name="$1"
  shift
  "$BIN" \
    "${common[@]}" \
    "$@" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "exactplay ${name} deep wide vs standard"
}

cfg="${1:-all}"
case "$cfg" in
  wide4_p1)
    run_one h2h_wide4_p1 --games 5 --seed 1 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" --p1-search "$WIDE"
    ;;
  wide4_p2)
    run_one h2h_wide4_p2 --games 5 --seed 1 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" --p2-search "$WIDE"
    ;;
  wide4_p1_s11)
    run_one h2h_wide4_p1_s11 --games 3 --seed 11 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" --p1-search "$WIDE"
    ;;
  wide4_p2_s11)
    run_one h2h_wide4_p2_s11 --games 3 --seed 11 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" --p2-search "$WIDE"
    ;;
  wide8_p1)
    run_one h2h_wide8_p1 --games 2 --seed 1 \
      --p1-budget "$DEEP8" --p2-budget "$DEEP8" --p1-search "$WIDE"
    ;;
  wide8_p2)
    run_one h2h_wide8_p2 --games 2 --seed 1 \
      --p1-budget "$DEEP8" --p2-budget "$DEEP8" --p2-search "$WIDE"
    ;;
  all)
    run_one h2h_wide4_p1 --games 5 --seed 1 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" --p1-search "$WIDE"
    run_one h2h_wide4_p2 --games 5 --seed 1 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" --p2-search "$WIDE"
    run_one h2h_wide4_p1_s11 --games 3 --seed 11 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" --p1-search "$WIDE"
    run_one h2h_wide4_p2_s11 --games 3 --seed 11 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" --p2-search "$WIDE"
    ;;
  *)
    echo "usage: $0 [all|wide4_p1|wide4_p2|wide4_p1_s11|wide4_p2_s11|wide8_p1|wide8_p2]" >&2
    exit 2
    ;;
esac
