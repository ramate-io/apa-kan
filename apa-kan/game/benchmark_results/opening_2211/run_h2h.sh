#!/usr/bin/env bash
# 2-2-1-1 opening (`--opening-double on` / `--opening 2-2-1-1`): P1 two
# consecutive 1-action turns, then P2 two, then singles. Off by default.
# Same light exactplay minimax-vs-minimax package as
# cascade_winner_swap/mirror/. Headline arm is ratchet on + 2-2-1-1.
#
# Usage:
#   run_h2h.sh r1            # 300-game mm-mm, ratchet + 2-2-1-1 (--seed 300)
#   run_h2h.sh baseline      # 300-game mm-mm, live rules + 2-2-1-1 (no ratchet)
#   run_h2h.sh all           # both
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/opening_2211"
mkdir -p "$out"

BIN="${APA_KAN_BENCH:-target/release/apa-kan-bench}"
if [[ ! -x "$BIN" ]]; then
  cargo build --release --no-default-features -p apa-kan --bin apa-kan-bench
  BIN="target/release/apa-kan-bench"
fi

LIGHT="nodes:33000,iters:650"
N300_SEED=300

common=(
  --zags on
  --zag-mismatch sum
  --endgame-rule control-cascade
  --cascade-turn-gain on
  --capture-mode local-claim
  --no-same-piece-twice off
  --pick search
  --pairings minimax:minimax,minimax:minimax
  --same-deck on
  --search-profile exactplay
  --jobs 4
  --no-depth-probe
  --opening-double on
  --budget "$LIGHT"
  --games 150
  --seed "$N300_SEED"
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
    --label "opening_2211 ${name}"
}

cfg="${1:-r1}"
case "$cfg" in
  r1)
    run_one h2h_n300_mm_r1 \
      --cascade-winner-swap on --ratchet-cascade on --extra-convert-mode any
    ;;
  baseline)
    run_one h2h_n300_mm_baseline
    ;;
  all)
    run_one h2h_n300_mm_r1 \
      --cascade-winner-swap on --ratchet-cascade on --extra-convert-mode any
    run_one h2h_n300_mm_baseline
    ;;
  *)
    echo "usage: $0 [r1|baseline|all]" >&2
    exit 2
    ;;
esac
