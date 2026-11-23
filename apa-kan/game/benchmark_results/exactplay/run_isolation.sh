#!/usr/bin/env bash
# Isolate eval_margin_scale=200 (exactplay) from enhanced (100), and the
# counterplay center-weight (eval_center_occ 70 vs 180) from enhanced.
#
# Usage:
#   run_isolation.sh all            # both exact-vs-enhanced seats + cheap counter-vs-enhanced
#   run_isolation.sh exact_p1       # P1 exactplay vs P2 enhanced (100 games)
#   run_isolation.sh exact_p2       # P1 enhanced vs P2 exactplay (100 games)
#   run_isolation.sh counter_p1     # P1 counterplay vs P2 enhanced (50 games)
#   run_isolation.sh counter_p2     # P1 enhanced vs P2 counterplay (50 games)
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/exactplay"
mkdir -p "$out"

BIN="${APA_KAN_BENCH:-target/release/apa-kan-bench}"
if [[ ! -x "$BIN" ]]; then
  cargo build --release --no-default-features -p apa-kan --bin apa-kan-bench
  BIN="target/release/apa-kan-bench"
fi

common=(
  --budget nodes:33000,iters:650
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
  local games="$2"
  shift 2
  "$BIN" \
    "${common[@]}" \
    --games "$games" \
    "$@" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "exactplay ${name} isolation"
}

cfg="${1:-all}"
case "$cfg" in
  exact_p1)
    run_one h2h_exact_vs_enh_p1 50 --p1-search-profile exactplay --p2-search-profile enhanced
    ;;
  exact_p2)
    run_one h2h_exact_vs_enh_p2 50 --p1-search-profile enhanced --p2-search-profile exactplay
    ;;
  counter_p1)
    run_one h2h_counter_vs_enh_p1 25 --p1-search-profile counterplay --p2-search-profile enhanced
    ;;
  counter_p2)
    run_one h2h_counter_vs_enh_p2 25 --p1-search-profile enhanced --p2-search-profile counterplay
    ;;
  all)
    run_one h2h_exact_vs_enh_p1 50 --p1-search-profile exactplay --p2-search-profile enhanced
    run_one h2h_exact_vs_enh_p2 50 --p1-search-profile enhanced --p2-search-profile exactplay
    run_one h2h_counter_vs_enh_p1 25 --p1-search-profile counterplay --p2-search-profile enhanced
    run_one h2h_counter_vs_enh_p2 25 --p1-search-profile enhanced --p2-search-profile counterplay
    ;;
  *)
    echo "usage: $0 [all|exact_p1|exact_p2|counter_p1|counter_p2]" >&2
    exit 2
    ;;
esac
