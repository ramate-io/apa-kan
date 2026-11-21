#!/usr/bin/env bash
# Distill cheap_order knobs from 4x logs, then (if the fit moves) confirm
# at light self-play and 4x vs the candidate.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$repo"
here="apa-kan/game/benchmark_results/exactplay"
work="$here/distill"
mkdir -p "$work"

BIN="${APA_KAN_BENCH:-target/release/apa-kan-bench}"
if [[ ! -x "$BIN" ]]; then
  cargo build --release --no-default-features -p apa-kan --bin apa-kan-bench
  BIN="target/release/apa-kan-bench"
fi

cfg="${1:-all}"
LIGHT="nodes:33000,iters:650"
DEEP4="nodes:132000,iters:2600"

compare() {
  "$BIN" \
    --compare-roots "$here/h2h_deep4x_self.jsonl.gz" \
    --search-profile exactplay \
    --budget "$LIGHT" \
    --jobs 4 \
    --out "$work/roots_deep4x_self.jsonl" \
    --no-games-log \
    --no-depth-probe
  gzip -f "$work/roots_deep4x_self.jsonl"
  python3 "$here/distill_order.py" fit \
    --roots "$work/roots_deep4x_self.jsonl.gz" \
    --out-toml "$work/candidate.toml" \
    --report "$work/fit_report.json"
}

common_h2h() {
  echo \
    --budget "$LIGHT" \
    --games 10 \
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
    --seed 1
}

confirm_self() {
  "$BIN" \
    $(common_h2h) \
    --bot-config "$work/candidate.toml" \
    --out "$work/h2h_cand_self.txt" \
    --record "$work/h2h_cand_self_record.txt" \
    --games-log "$work/h2h_cand_self.jsonl.gz" \
    --label "distill candidate self light"
}

confirm_vs4x() {
  "$BIN" \
    $(common_h2h) \
    --search-profile exactplay \
    --p2-bot-config "$work/candidate.toml" \
    --p1-budget "$DEEP4" --p2-budget "$LIGHT" \
    --games 5 \
    --out "$work/h2h_deep4x_vs_cand_p1.txt" \
    --record "$work/h2h_deep4x_vs_cand_p1_record.txt" \
    --games-log "$work/h2h_deep4x_vs_cand_p1.jsonl.gz" \
    --label "4x exactplay vs distill-light P2"
  "$BIN" \
    $(common_h2h) \
    --search-profile exactplay \
    --p1-bot-config "$work/candidate.toml" \
    --p1-budget "$LIGHT" --p2-budget "$DEEP4" \
    --games 5 \
    --out "$work/h2h_deep4x_vs_cand_p2.txt" \
    --record "$work/h2h_deep4x_vs_cand_p2_record.txt" \
    --games-log "$work/h2h_deep4x_vs_cand_p2.jsonl.gz" \
    --label "distill-light P1 vs 4x exactplay"
}

case "$cfg" in
  compare) compare ;;
  self) confirm_self ;;
  vs4x) confirm_vs4x ;;
  all)
    compare
    ;;
  *)
    echo "usage: $0 [all|compare|self|vs4x]" >&2
    exit 2
    ;;
esac
