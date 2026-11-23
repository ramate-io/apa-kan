#!/usr/bin/env bash
# Capture-score overnight cell. Live control stays the default; this script
# benches `--objective capture-score` and the three independent toggles
# against a control baseline. Liam-standard light package: release,
# --no-default-features, nodes:33000,iters:650, exactplay, zag-mismatch sum.
#
# Usage:
#   run_h2h.sh smoke            # 4-game mm-mm sanity (both objectives)
#   run_h2h.sh control_mm       # 150-game mm-mm on live control defaults
#   run_h2h.sh control_mcts     # 100-game mm-mcts on live control defaults
#   run_h2h.sh base_mm          # 150-game mm-mm capture-score (no toggles)
#   run_h2h.sh base_mcts        # 100-game mm-mcts capture-score
#   run_h2h.sh majority_mm      # variant 4
#   run_h2h.sh cascade_mm       # variant 5
#   run_h2h.sh unlimited_mm     # variant 6
#   run_h2h.sh majority_mcts | cascade_mcts | unlimited_mcts
#   run_h2h.sh combo_maj_cas_mm # 4+5
#   run_h2h.sh combo_maj_unlim_mm # 4+6
#   run_h2h.sh combo_maj_cas_unlim_mm # 4+5+6
#   run_h2h.sh majority_inner_mm | majority_center_mm  # trigger sweep
#   run_h2h.sh majority_t4_mm | majority_t8_mm         # score-target sweep
#   run_h2h.sh majority_w40_mm | majority_w80_mm | majority_w160_mm
#   run_h2h.sh weight400_mm     # base + eval_capture_score=400
#   run_h2h.sh all_light        # control + base + 4/5/6 mm-mm (150 each)
#   run_h2h.sh followup_maj     # combos + trigger/target/weight sweeps
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
    run_one smoke_base "${mm_pair[@]}" --games 2 --seed 1 --objective capture-score
    ;;
  control_mm)
    run_one h2h_control_mm "${mm_pair[@]}" --games 75 --seed 300 --objective control
    ;;
  control_mcts)
    run_one h2h_control_mcts "${mcts_pair[@]}" --games 50 --seed 10300 --objective control
    ;;
  base_mm)
    run_one h2h_base_mm "${mm_pair[@]}" --games 75 --seed 300 --objective capture-score
    ;;
  base_mcts)
    run_one h2h_base_mcts "${mcts_pair[@]}" --games 50 --seed 10300 --objective capture-score
    ;;
  majority_mm)
    run_one h2h_majority_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --objective capture-score --majority-spawn-bonus on
    ;;
  majority_mcts)
    run_one h2h_majority_mcts "${mcts_pair[@]}" --games 50 --seed 10300 \
      --objective capture-score --majority-spawn-bonus on
    ;;
  cascade_mm)
    run_one h2h_cascade_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --objective capture-score --capture-cascade on
    ;;
  cascade_mcts)
    run_one h2h_cascade_mcts "${mcts_pair[@]}" --games 50 --seed 10300 \
      --objective capture-score --capture-cascade on
    ;;
  unlimited_mm)
    run_one h2h_unlimited_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --objective capture-score --unlimited-spawn on
    ;;
  unlimited_mcts)
    run_one h2h_unlimited_mcts "${mcts_pair[@]}" --games 50 --seed 10300 \
      --objective capture-score --unlimited-spawn on
    ;;
  combo_maj_cas_mm)
    run_one h2h_maj_cas_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --objective capture-score --majority-spawn-bonus on --capture-cascade on
    ;;
  combo_maj_unlim_mm)
    run_one h2h_maj_unlim_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --objective capture-score --majority-spawn-bonus on --unlimited-spawn on
    ;;
  combo_maj_cas_unlim_mm)
    run_one h2h_maj_cas_unlim_mm "${mm_pair[@]}" --games 40 --seed 500 \
      --objective capture-score --majority-spawn-bonus on --capture-cascade on --unlimited-spawn on
    ;;
  majority_inner_mm)
    run_one h2h_majority_inner_mm "${mm_pair[@]}" --games 40 --seed 400 \
      --objective capture-score --majority-spawn-bonus on --majority-spawn-set inner
    ;;
  majority_center_mm)
    run_one h2h_majority_center_mm "${mm_pair[@]}" --games 40 --seed 400 \
      --objective capture-score --majority-spawn-bonus on --majority-spawn-set center
    ;;
  majority_t4_mm)
    run_one h2h_majority_t4_mm "${mm_pair[@]}" --games 40 --seed 400 \
      --objective capture-score --majority-spawn-bonus on --score-target 4
    ;;
  majority_t8_mm)
    run_one h2h_majority_t8_mm "${mm_pair[@]}" --games 40 --seed 400 \
      --objective capture-score --majority-spawn-bonus on --score-target 8
    ;;
  majority_w40_mm)
    run_one h2h_majority_w40_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --majority-spawn-bonus on --search eval-capture-score=40
    ;;
  majority_w80_mm)
    run_one h2h_majority_w80_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --majority-spawn-bonus on --search eval-capture-score=80
    ;;
  majority_w160_mm)
    run_one h2h_majority_w160_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --majority-spawn-bonus on --search eval-capture-score=160
    ;;
  weight40_mm)
    run_one h2h_weight40_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --search eval-capture-score=40
    ;;
  weight80_mm)
    run_one h2h_weight80_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --search eval-capture-score=80
    ;;
  weight160_mm)
    run_one h2h_weight160_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --search eval-capture-score=160
    ;;
  weight400_mm)
    run_one h2h_weight400_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --objective capture-score --search eval-capture-score=400
    ;;
  followup_maj)
    run_one h2h_maj_unlim_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --objective capture-score --majority-spawn-bonus on --unlimited-spawn on
    run_one h2h_majority_inner_mm "${mm_pair[@]}" --games 40 --seed 400 \
      --objective capture-score --majority-spawn-bonus on --majority-spawn-set inner
    run_one h2h_majority_center_mm "${mm_pair[@]}" --games 40 --seed 400 \
      --objective capture-score --majority-spawn-bonus on --majority-spawn-set center
    run_one h2h_majority_t4_mm "${mm_pair[@]}" --games 40 --seed 400 \
      --objective capture-score --majority-spawn-bonus on --score-target 4
    run_one h2h_majority_t8_mm "${mm_pair[@]}" --games 40 --seed 400 \
      --objective capture-score --majority-spawn-bonus on --score-target 8
    run_one h2h_majority_w40_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --majority-spawn-bonus on --search eval-capture-score=40
    run_one h2h_majority_w80_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --majority-spawn-bonus on --search eval-capture-score=80
    run_one h2h_majority_w160_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --majority-spawn-bonus on --search eval-capture-score=160
    run_one h2h_weight40_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --search eval-capture-score=40
    run_one h2h_weight80_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --search eval-capture-score=80
    run_one h2h_weight160_mm "${mm_pair[@]}" --games 20 --seed 600 \
      --objective capture-score --search eval-capture-score=160
    run_one h2h_maj_cas_unlim_mm "${mm_pair[@]}" --games 40 --seed 500 \
      --objective capture-score --majority-spawn-bonus on --capture-cascade on --unlimited-spawn on
    ;;
  all_light)
    run_one h2h_control_mm "${mm_pair[@]}" --games 75 --seed 300 --objective control
    run_one h2h_base_mm "${mm_pair[@]}" --games 75 --seed 300 --objective capture-score
    run_one h2h_majority_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --objective capture-score --majority-spawn-bonus on
    run_one h2h_cascade_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --objective capture-score --capture-cascade on
    run_one h2h_unlimited_mm "${mm_pair[@]}" --games 75 --seed 300 \
      --objective capture-score --unlimited-spawn on
    ;;
  *)
    echo "usage: $0 [smoke|control_mm|control_mcts|base_mm|base_mcts|majority_mm|majority_mcts|cascade_mm|cascade_mcts|unlimited_mm|unlimited_mcts|combo_maj_cas_mm|combo_maj_unlim_mm|combo_maj_cas_unlim_mm|majority_inner_mm|majority_center_mm|majority_t4_mm|majority_t8_mm|majority_w40_mm|majority_w80_mm|majority_w160_mm|weight40_mm|weight80_mm|weight160_mm|weight400_mm|followup_maj|all_light]" >&2
    exit 2
    ;;
esac
