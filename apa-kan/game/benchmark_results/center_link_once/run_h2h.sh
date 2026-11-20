#!/usr/bin/env bash
# Same-deck 27 shallow H2H: link-once ON, ColorPass.drift 160.
# Matrix: {4:3:1, 7:5:4} × {independent-max, additive}, current braid scoring.
# Braid variants: braid-only-match / exact / double-edged on additive.
#
# Usage:
#   run_h2h.sh matrix
#   run_h2h.sh braid [431_add|754_add|all]
#   run_h2h.sh 431_imax|431_add|754_imax|754_add
#   run_h2h.sh 431_add_bom|431_add_exact|431_add_de|754_add_bom|754_add_exact|754_add_de
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
  431_add_bom) run_one h2h_431_add_bom off 4,3,1 --braid-scoring braid-only-match ;;
  431_add_exact) run_one h2h_431_add_exact off 4,3,1 --braid-scoring exact ;;
  431_add_de) run_one h2h_431_add_de off 4,3,1 --braid-scoring double-edged ;;
  754_add_bom) run_one h2h_754_add_bom off 7,5,4 --braid-scoring braid-only-match ;;
  754_add_exact) run_one h2h_754_add_exact off 7,5,4 --braid-scoring exact ;;
  754_add_de) run_one h2h_754_add_de off 7,5,4 --braid-scoring double-edged ;;
  matrix)
    run_one h2h_431_imax on 4,3,1
    run_one h2h_431_add off 4,3,1
    run_one h2h_754_imax on 7,5,4
    run_one h2h_754_add off 7,5,4
    ;;
  braid)
    cell="${2:-all}"
    case "$cell" in
      431_add)
        run_one h2h_431_add_bom off 4,3,1 --braid-scoring braid-only-match
        run_one h2h_431_add_exact off 4,3,1 --braid-scoring exact
        run_one h2h_431_add_de off 4,3,1 --braid-scoring double-edged
        ;;
      754_add)
        run_one h2h_754_add_bom off 7,5,4 --braid-scoring braid-only-match
        run_one h2h_754_add_exact off 7,5,4 --braid-scoring exact
        run_one h2h_754_add_de off 7,5,4 --braid-scoring double-edged
        ;;
      all)
        run_one h2h_431_add_bom off 4,3,1 --braid-scoring braid-only-match
        run_one h2h_431_add_exact off 4,3,1 --braid-scoring exact
        run_one h2h_431_add_de off 4,3,1 --braid-scoring double-edged
        run_one h2h_754_add_bom off 7,5,4 --braid-scoring braid-only-match
        run_one h2h_754_add_exact off 7,5,4 --braid-scoring exact
        run_one h2h_754_add_de off 7,5,4 --braid-scoring double-edged
        ;;
      *)
        echo "usage: $0 braid [431_add|754_add|all]" >&2
        exit 2
        ;;
    esac
    ;;
  *)
    echo "usage: $0 [matrix|braid|431_imax|431_add|754_imax|754_add|431_add_bom|...]" >&2
    exit 2
    ;;
esac
