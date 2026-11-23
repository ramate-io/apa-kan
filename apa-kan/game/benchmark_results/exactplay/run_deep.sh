#!/usr/bin/env bash
# Deep-search probe: does sitting on center win only because the light
# budget is shallow? exactplay vs itself at 4x / 8x, and 4x vs light,
# both seats. Search-pick stays on the light budget so draft matches the
# committed 100-game self-play cell.
#
# Usage:
#   run_deep.sh all
#   run_deep.sh self4 | vs4_p1 | vs4_p2 | self8
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

LIGHT="nodes:33000,iters:650"
DEEP4="nodes:132000,iters:2600"
DEEP8="nodes:264000,iters:5200"

common=(
  --budget "$LIGHT"
  --games 5
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
    --label "exactplay ${name} deep probe"
}

cfg="${1:-all}"
case "$cfg" in
  self4)
    run_one h2h_deep4x_self --p1-budget "$DEEP4" --p2-budget "$DEEP4"
    ;;
  vs4_p1)
    run_one h2h_deep4x_vs_light_p1 --p1-budget "$DEEP4" --p2-budget "$LIGHT"
    ;;
  vs4_p2)
    run_one h2h_deep4x_vs_light_p2 --p1-budget "$LIGHT" --p2-budget "$DEEP4"
    ;;
  self8)
    run_one h2h_deep8x_self --p1-budget "$DEEP8" --p2-budget "$DEEP8"
    ;;
  all)
    run_one h2h_deep4x_self --p1-budget "$DEEP4" --p2-budget "$DEEP4"
    run_one h2h_deep4x_vs_light_p1 --p1-budget "$DEEP4" --p2-budget "$LIGHT"
    run_one h2h_deep4x_vs_light_p2 --p1-budget "$LIGHT" --p2-budget "$DEEP4"
    run_one h2h_deep8x_self --p1-budget "$DEEP8" --p2-budget "$DEEP8"
    ;;
  *)
    echo "usage: $0 [all|self4|vs4_p1|vs4_p2|self8]" >&2
    exit 2
    ;;
esac
