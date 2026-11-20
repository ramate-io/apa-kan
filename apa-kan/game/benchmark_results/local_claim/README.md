# Local-claim captures (shallow H2H)

Compares `--capture-mode local-claim` (this branch's default: ADD/MOVE
capture only dest + neighbors; leftover overwhelmed tiles stay until
healed or Claimed) with PR #70 cascading resolution. Independent-max
channels, doubles allowed, zags on, `zag_mismatch=sum`. Do not
overwrite; quote numbers from the committed `*.txt` logs in this
directory.

## Package

Release `apa-kan-bench`, `--budget nodes:33000,iters:650` (exactly one
quarter of calibrated `nodes:132000,iters:2600`), `--pick search`,
`--jobs 4`, `--no-depth-probe`, `--no-same-piece-twice off`, `--zags on`,
`--channel-mode independent-max`, `--capture-mode local-claim`,
`--zag-mismatch sum`, `--pairings minimax:mcts,mcts:minimax`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`,
`--games 10` (100 games).

| File | Games | Budget |
|---|---|---|
| [`h2h_shallow.txt`](h2h_shallow.txt) | 100 | nodes:33000,iters:650 |

Per-game JSONL: [`h2h_shallow.jsonl.gz`](h2h_shallow.jsonl.gz). Win/draw
split of every end reason: [`ends_breakdown.txt`](ends_breakdown.txt)
(from [`summarize_ends.py`](summarize_ends.py)).

Comparison baseline: PR #70
[`../independent_max/h2h_shallow.txt`](../independent_max/h2h_shallow.txt).

## Summary (quoted)

From [`h2h_shallow.txt`](h2h_shallow.txt):

```
# rules: ... capture_mode=local-claim ... channel_mode=independent-max ... zag_mismatch=sum ...
ADD       364          42.38  88
MOVE      381          17.35  54
CLAIM     10           1.00   1
ALL       420          52.50  110
# seat_totals  P1 40-6-54  P2 54-6-40  games=100
# end_reasons  elim=0 locked_out=17 center_hold=0 petal_maj=0 control=18 insuff=0 low_value=61 low_value_wins=59 low_value_draws=2 turn_limit=4 double_pass=0
# captures  opponent_tiles_removed=621  avg_per_game=6.21  self_captures=758  self_avg_per_game=7.58
# claims  total=2 avg_per_game=0.02  own=0 opponent=2  healed=4 healed_avg_per_game=0.04
# search_depth  mm_avg=3.36 mm_max=16 mm_n=2823  mcts_avg=1.74 mcts_max=7 mcts_n=2815
# matchup_totals  mixed vs mixed 7-1-12 avg_turns=53.15  |  braid vs braid 4-2-14 avg_turns=59.80  |  bead vs bead 8-1-11 avg_turns=57.20  |  braid vs bead 8-1-11 avg_turns=54.50  |  bead vs braid 13-1-6 avg_turns=57.25
# elapsed_sec=750.1
```

Unweighted mean of the ten pairing `avg_turns` columns: **56.40**.

JSONL win/draw split ([`ends_breakdown.txt`](ends_breakdown.txt)):
elim 0-0, locked_out 17-0, control 18-0, low_value 59-2, turn_limit 0-4;
center_hold / petal_majority / insufficient_tiles / double_pass all 0.

## vs PR #70 cascade (`independent_max/h2h_shallow.txt`)

Same budget, pairings, matchups, seed_base=1.

| | local-claim | PR #70 cascade |
|---|---|---|
| P1 W-D-L | 40-6-54 | 41-6-53 |
| elim / lock / control / LV / TL | 0 / 17 / 18 / 61 / 4 | 1 / 23 / 20 / 51 / 5 |
| opp captures /game | 6.21 (621) | 6.40 (640) |
| self captures /game | 7.58 (758) | 7.24 (724) |
| claims (own / opp) | 2 (0 / 2) | n/a (cascade) |
| heals | 4 | n/a |
| mm_avg / mm_max | 3.36 / 16 | 3.37 / 16 |
| mcts_avg / mcts_max | 1.74 / 7 | 1.73 / 6 |
| mean pairing avg_turns | 56.40 | 59.87 |
| elapsed_sec | 750.1 | 756.8 |
