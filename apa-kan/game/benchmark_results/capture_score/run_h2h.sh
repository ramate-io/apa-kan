#!/usr/bin/env bash
# Capture-score benchmark cells. Live control stays the default; the
# capture-score game is one preset, `--ruleset capture-b` (see RULES.md).
# Liam-standard light package: release, --no-default-features,
# nodes:33000,iters:650, exactplay, zag-mismatch sum. 75 deals x 2 seats.
#
# Usage:
#   run_h2h.sh smoke            # 2-game mm-mm sanity per objective
#   run_h2h.sh control_mm       # 150-game mm-mm on live control defaults
#   run_h2h.sh capture_b_smoke  # 4-game mm-mm sanity of the preset
#   run_h2h.sh capture_b_mm     # 150-game mm-mm of the preset, seed 300
#   run_h2h.sh capture_b_penalty_always_mm | capture_b_penalty_matchpoint_mm
#
# The sweep cells behind the committed h2h_* logs (majority bonus, capture
# cascade, weights, graveyard / capturer ablations, mark modes A and
# A-discard, the separate no-return and repetition cells) used flags that
# were removed; their commands are in this file's history before the
# simplification (`git log -- run_h2h.sh`).
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/capture_score"
mkdir -p "$out"

BIN="${APA_KAN_BENCH:-target/release/apa-kan-bench}"
if [[ ! -x "$BIN" ]]; then
  cargo build --release --no-default-features -p apa-kan --bin apa-kan-bench
  BIN="target/release/apa-kan-bench"
fi

LIGHT="nodes:33000,iters:650"

common=(
  --zags on
  --zag-mismatch sum
  --endgame-rule control-cascade
  --cascade-turn-gain on
  --cascade-winner-swap on
  --ratchet-cascade on
  --capture-mode local-claim
  --no-same-piece-twice off
  --opening-double on
  --pick search
  --same-deck on
  --search-profile exactplay
  --jobs 4
  --no-depth-probe
  --budget "$LIGHT"
)

mm_pair=(--pairings minimax:minimax,minimax:minimax)
mcts_pair=(--pairings minimax:mcts,mcts:minimax)

run_one() {
  local name="$1"
  shift
  echo "==> $name $*"
  "$BIN" \
    "${common[@]}" \
    "$@" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "capture_score ${name}"
}

cfg="${1:-smoke}"
case "$cfg" in
  smoke)
    run_one smoke_control "${mm_pair[@]}" --games 2 --seed 1 --objective control
    run_one smoke_capture_b "${mm_pair[@]}" --games 2 --seed 1 --ruleset capture-b
    ;;
  control_mm)
    run_one h2h_control_mm "${mm_pair[@]}" --games 75 --seed 300 --objective control
    ;;
  capture_b_smoke)
    run_one smoke_capture_b "${mm_pair[@]}" --games 2 --seed 1 --ruleset capture-b
    ;;
  capture_b_mm)
    run_one h2h_capture_b_mm "${mm_pair[@]}" --games 75 --seed 300 --ruleset capture-b
    ;;
  capture_b_penalty_always_mm)
    run_one h2h_capture_b_penalty_always_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --ruleset capture-b --self-capture-penalty always
    ;;
  capture_b_penalty_matchpoint_mm)
    run_one h2h_capture_b_penalty_matchpoint_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --ruleset capture-b --self-capture-penalty match-point
    ;;
  *)
    echo "usage: $0 [smoke|control_mm|capture_b_smoke|capture_b_mm|capture_b_penalty_always_mm|capture_b_penalty_matchpoint_mm]" >&2
    exit 2
    ;;
esac
