#!/usr/bin/env bash
# Shallow H2H for control-cascade turn-gain. Same budget/matchups as PR #74.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
out="apa-kan/game/benchmark_results/control_cascade_turn_gain"
mkdir -p "$out"
exec cargo run --release --no-default-features -p apa-kan --bin apa-kan-bench -- \
  --budget nodes:33000,iters:650 \
  --games 10 \
  --jobs 4 \
  --no-depth-probe \
  --zags on \
  --zag-mismatch sum \
  --channel-mode independent-max \
  --endgame-rule control-cascade \
  --cascade-turn-gain on \
  --capture-mode local-claim \
  --no-same-piece-twice off \
  --pick search \
  --pairings minimax:mcts,mcts:minimax \
  --deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid \
  --seed 1 \
  --out "$out/h2h_shallow.txt" \
  --record "$out/h2h_shallow_record.txt" \
  --games-log "$out/h2h_shallow.jsonl.gz" \
  --label "control-cascade turn-gain shallow H2H"
