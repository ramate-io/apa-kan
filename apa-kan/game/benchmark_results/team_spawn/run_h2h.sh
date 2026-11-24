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
out="apa-kan/game/benchmark_results/team_spawn"
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
    --label "team_spawn ${name}"
}

# Every cell: capture-b + team spawn diamonds + a singles-heavy collection.
base=(--ruleset capture-b --team-spawn on --ring-count-weights 10,3,1)
L1=(--move-support matched)
L3=(--match-gain minus-rings)
CAP=(--bench-ring-cap 17)
PERM=(--mirror-palette permute)
FLIP=(--surround-flip on)

cell() {
  local name="$1"
  shift
  run_one "h2h_${name}_mm" "${mm_pair[@]}" --games 75 --seed 300 "${base[@]}" "$@"
}

cfg="${1:-smoke}"
case "$cfg" in
  smoke) run_one smoke_ts "${mm_pair[@]}" --games 2 --seed 1 "${base[@]}" "${L1[@]}" "${L3[@]}" "${CAP[@]}" "${FLIP[@]}" ;;
  base) cell ts_base ;;
  l1) cell ts_l1 "${L1[@]}" ;;
  l3) cell ts_l3 "${L3[@]}" ;;
  combo) cell ts_l1_l3_cap17 "${L1[@]}" "${L3[@]}" "${CAP[@]}" ;;
  base_perm) cell ts_base_perm "${PERM[@]}" ;;
  l1_perm) cell ts_l1_perm "${L1[@]}" "${PERM[@]}" ;;
  l3_perm) cell ts_l3_perm "${L3[@]}" "${PERM[@]}" ;;
  combo_perm) cell ts_l1_l3_cap17_perm "${L1[@]}" "${L3[@]}" "${CAP[@]}" "${PERM[@]}" ;;
  combo_flip) cell ts_l1_l3_cap17_flip "${L1[@]}" "${L3[@]}" "${CAP[@]}" "${FLIP[@]}" ;;
  combo_flip_perm) cell ts_l1_l3_cap17_flip_perm "${L1[@]}" "${L3[@]}" "${CAP[@]}" "${FLIP[@]}" "${PERM[@]}" ;;
  *)
    echo "usage: $0 [smoke|base|l1|l3|combo|base_perm|l1_perm|l3_perm|combo_perm|combo_flip|combo_flip_perm]" >&2
    exit 2
    ;;
esac
