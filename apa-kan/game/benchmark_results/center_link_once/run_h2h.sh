#!/usr/bin/env bash
# Archived: `--link-once` / `--braid-scoring` CLI were removed. Do not re-run
# against current binaries. Logs in this directory still replay.
# Same-deck 27 shallow H2H: link-once ON, ColorPass.drift 160.
# Two matrices, each (4:3:1, 7:5:4) × {independent-max, additive}:
#   (a) current braid scoring
#   (b) braid-only-match
#
# Usage:
#   run_h2h.sh matrix          # current scoring, 4 cells
#   run_h2h.sh braid           # braid-only-match, 4 cells
#   run_h2h.sh all             # both matrices
#   run_h2h.sh 431_imax|431_add|754_imax|754_add
#   run_h2h.sh 431_imax_bom|431_add_bom|754_imax_bom|754_add_bom
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/center_link_once"
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
  --search-profile counterplay
  --seed 1
  --color-drift 160
  --link-once on
)

run_one() {
  local name="$1"
  local channels="$2"
  local weights="$3"
  shift 3
  cargo run --release --offline --no-default-features -p apa-kan --bin apa-kan-bench -- \
    "${common[@]}" \
    --independent-channels "$channels" \
    --ring-count-weights "$weights" \
    "$@" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "link-once drift160 ${name} shallow H2H"
}

cfg="${1:-matrix}"
case "$cfg" in
  431_imax) run_one h2h_431_imax on 4,3,1 ;;
  431_add) run_one h2h_431_add off 4,3,1 ;;
  754_imax) run_one h2h_754_imax on 7,5,4 ;;
  754_add) run_one h2h_754_add off 7,5,4 ;;
  431_imax_bom) run_one h2h_431_imax_bom on 4,3,1 --braid-scoring braid-only-match ;;
  431_add_bom) run_one h2h_431_add_bom off 4,3,1 --braid-scoring braid-only-match ;;
  754_imax_bom) run_one h2h_754_imax_bom on 7,5,4 --braid-scoring braid-only-match ;;
  754_add_bom) run_one h2h_754_add_bom off 7,5,4 --braid-scoring braid-only-match ;;
  matrix)
    run_one h2h_431_imax on 4,3,1
    run_one h2h_431_add off 4,3,1
    run_one h2h_754_imax on 7,5,4
    run_one h2h_754_add off 7,5,4
    ;;
  braid)
    run_one h2h_431_imax_bom on 4,3,1 --braid-scoring braid-only-match
    run_one h2h_431_add_bom off 4,3,1 --braid-scoring braid-only-match
    run_one h2h_754_imax_bom on 7,5,4 --braid-scoring braid-only-match
    run_one h2h_754_add_bom off 7,5,4 --braid-scoring braid-only-match
    ;;
  all)
    run_one h2h_431_imax on 4,3,1
    run_one h2h_431_add off 4,3,1
    run_one h2h_754_imax on 7,5,4
    run_one h2h_754_add off 7,5,4
    run_one h2h_431_imax_bom on 4,3,1 --braid-scoring braid-only-match
    run_one h2h_431_add_bom off 4,3,1 --braid-scoring braid-only-match
    run_one h2h_754_imax_bom on 7,5,4 --braid-scoring braid-only-match
    run_one h2h_754_add_bom off 7,5,4 --braid-scoring braid-only-match
    ;;
  *)
    echo "usage: $0 [all|matrix|braid|431_imax|431_add|754_imax|754_add|431_imax_bom|431_add_bom|754_imax_bom|754_add_bom]" >&2
    exit 2
    ;;
esac
