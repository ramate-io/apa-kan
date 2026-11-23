#!/usr/bin/env bash
# Archived: `--link-once` / `--braid-scoring` CLI were removed. Do not re-run
# against current binaries. Logs in this directory still replay.
# Full counterplay package: 4:3:1 Additive, braid-only-exact, single braids,
# normal-only singles-swap. Cells: link-once vs double-count, plus optional
# ablations (no-swap / no-single-braids) at one link setting.
#
# Usage:
#   run_h2h.sh all              # both link cells (explicit flags)
#   run_h2h.sh link_once
#   run_h2h.sh double
#   run_h2h.sh defaults         # live RuleSet defaults; no extra rule flags
#   run_h2h.sh noswap           # ablation: full setup, swaps off (pass LINK=on|off)
#   run_h2h.sh nobraid          # ablation: full setup, no single braids
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/single_braid_exact_swap"
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
  --independent-channels off
  --ring-count-weights 4,3,1
  --braid-scoring braid-only-exact
  --single-braids on
  --singles-swap normal-only
)

run_one() {
  local name="$1"
  shift
  cargo run --release --offline --no-default-features -p apa-kan --bin apa-kan-bench -- \
    "${common[@]}" \
    "$@" \
    --out "$out/${name}.txt" \
    --record "$out/${name}_record.txt" \
    --games-log "$out/${name}.jsonl.gz" \
    --label "single-braid-exact ${name} shallow H2H"
}

cfg="${1:-all}"
link="${LINK:-on}"
case "$cfg" in
  defaults)
    cargo run --release --offline --no-default-features -p apa-kan --bin apa-kan-bench -- \
      --budget nodes:33000,iters:650 \
      --games 50 \
      --jobs 4 \
      --no-depth-probe \
      --zags on \
      --zag-mismatch sum \
      --endgame-rule control-cascade \
      --cascade-turn-gain on \
      --capture-mode local-claim \
      --no-same-piece-twice off \
      --pick search \
      --pairings minimax:mcts,mcts:minimax \
      --same-deck on \
      --search-profile counterplay \
      --seed 1 \
      --out "$out/h2h_defaults.txt" \
      --record "$out/h2h_defaults_record.txt" \
      --games-log "$out/h2h_defaults.jsonl.gz" \
      --label "single-braid-exact h2h_defaults shallow H2H"
    ;;
  link_once)
    run_one h2h_full_link_once --link-once on
    ;;
  double)
    run_one h2h_full_double --link-once off
    ;;
  noswap)
    tag=$([[ "$link" == on ]] && echo link_once || echo double)
    run_one "h2h_full_${tag}_noswap" --link-once "$link" --singles-swap off
    ;;
  nobraid)
    tag=$([[ "$link" == on ]] && echo link_once || echo double)
    run_one "h2h_full_${tag}_nobraid" --link-once "$link" --single-braids off
    ;;
  all)
    run_one h2h_full_link_once --link-once on
    run_one h2h_full_double --link-once off
    ;;
  *)
    echo "usage: $0 [all|link_once|double|defaults|noswap|nobraid]  (noswap/nobraid: LINK=on|off)" >&2
    exit 2
    ;;
esac
