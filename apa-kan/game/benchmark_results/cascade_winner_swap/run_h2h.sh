#!/usr/bin/env bash
# Cascade-winner extra swap + center-mismatch exactplay self-play.
# Live RuleSet defaults otherwise (flags default off). Light budget matches
# PR #85 exactplay self-play.
#
# Usage:
#   run_h2h.sh all            # four light 100-game cells
#   run_h2h.sh baseline|r1|r2|both
#   run_h2h.sh deep_baseline|deep_r1|deep_r2|deep_both   # 4x, 20 games
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/cascade_winner_swap"
mkdir -p "$out"

BIN="${APA_KAN_BENCH:-target/release/apa-kan-bench}"
if [[ ! -x "$BIN" ]]; then
  cargo build --release --no-default-features -p apa-kan --bin apa-kan-bench
  BIN="target/release/apa-kan-bench"
fi

LIGHT="nodes:33000,iters:650"
DEEP4="nodes:132000,iters:2600"

common=(
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
  --jobs 4
  --no-depth-probe
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
    --label "cascade_winner_swap ${name}"
}

cfg="${1:-all}"
case "$cfg" in
  baseline)
    run_one h2h_baseline --budget "$LIGHT" --games 50
    ;;
  r1)
    run_one h2h_r1 --budget "$LIGHT" --games 50 --cascade-winner-swap on
    ;;
  r2)
    run_one h2h_r2 --budget "$LIGHT" --games 50 --center-mismatch on
    ;;
  both)
    run_one h2h_both --budget "$LIGHT" --games 50 --cascade-winner-swap on --center-mismatch on
    ;;
  deep_baseline)
    run_one h2h_deep4x_baseline --budget "$LIGHT" --games 10 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4"
    ;;
  deep_r1)
    run_one h2h_deep4x_r1 --budget "$LIGHT" --games 10 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" --cascade-winner-swap on
    ;;
  deep_r2)
    run_one h2h_deep4x_r2 --budget "$LIGHT" --games 10 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" --center-mismatch on
    ;;
  deep_both)
    run_one h2h_deep4x_both --budget "$LIGHT" --games 10 \
      --p1-budget "$DEEP4" --p2-budget "$DEEP4" \
      --cascade-winner-swap on --center-mismatch on
    ;;
  all)
    run_one h2h_baseline --budget "$LIGHT" --games 50
    run_one h2h_r1 --budget "$LIGHT" --games 50 --cascade-winner-swap on
    run_one h2h_r2 --budget "$LIGHT" --games 50 --center-mismatch on
    run_one h2h_both --budget "$LIGHT" --games 50 --cascade-winner-swap on --center-mismatch on
    ;;
  *)
    echo "usage: $0 [all|baseline|r1|r2|both|deep_baseline|deep_r1|deep_r2|deep_both]" >&2
    exit 2
    ;;
esac
