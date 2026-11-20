# Material-value endgame (shallow H2H)

`--endgame-rule material-value` (this branch's default) replaces the
immediate low-value tiebreaker when both players have
`board + usable spawn tokens < 5`. Resolution is three-tier: higher
total material, then more tiles on the board, then the existing pairwise
lowest-piece check. Control 5-of-9, lockout, elimination, and turn-limit
are unchanged. Local-claim captures and independent-max channels stay
on. Doubles allowed, zags on, `zag_mismatch=sum`. Do not overwrite;
quote numbers from the committed `*.txt` logs in this directory.

## Package

Release `apa-kan-bench`, `--budget nodes:33000,iters:650` (exactly one
quarter of calibrated `nodes:132000,iters:2600`), `--pick search`,
`--jobs 4`, `--no-depth-probe`, `--no-same-piece-twice off`, `--zags on`,
`--channel-mode independent-max`, `--capture-mode local-claim`,
`--endgame-rule material-value`, `--zag-mismatch sum`, `--pairings
minimax:mcts,mcts:minimax`, `--deck-matchups
mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`, `--games 10`
(100 games).

| File | Games | Budget |
|---|---|---|
| [`h2h_shallow.txt`](h2h_shallow.txt) | 100 | nodes:33000,iters:650 |

Per-game JSONL: [`h2h_shallow.jsonl.gz`](h2h_shallow.jsonl.gz). Win/draw
split of every end reason, plus material-value tiers:
[`ends_breakdown.txt`](ends_breakdown.txt) (from
[`summarize_ends.py`](summarize_ends.py)).

Baselines: PR #73
[`../local_claim/h2h_shallow.txt`](../local_claim/h2h_shallow.txt)
(same capture mode, previous `low-value` endgame) and PR #70
[`../independent_max/h2h_shallow.txt`](../independent_max/h2h_shallow.txt)
(cascade captures, previous `low-value` endgame).

## Summary (quoted)

From [`h2h_shallow.txt`](h2h_shallow.txt):

```
# rules: ... endgame_rule=material-value ... capture_mode=local-claim ... channel_mode=independent-max ... zag_mismatch=sum ...
ADD       358          42.75  88
MOVE      371          17.30  54
CLAIM     3            1.00   1
ALL       410          52.98  110
# seat_totals  P1 51-5-44  P2 44-5-51  games=100
# end_reasons  elim=0 locked_out=8 center_hold=0 petal_maj=0 control=20 insuff=0 low_value=0 low_value_wins=4 low_value_draws=2 material_value=69 material_value_wins=67 material_value_draws=2 turn_limit=3 double_pass=0
# material_value_tiers  total_material=49 board_tiles=14 low_value=4 draws=2
# captures  opponent_tiles_removed=552  avg_per_game=5.52  self_captures=703  self_avg_per_game=7.03
# claims  total=2 avg_per_game=0.02  own=0 opponent=2  healed=0 healed_avg_per_game=0.00
# search_depth  mm_avg=3.19 mm_max=16 mm_n=2653  mcts_avg=1.72 mcts_max=7 mcts_n=2650
# matchup_totals  mixed vs mixed 8-1-11 avg_turns=48.05  |  braid vs braid 10-0-10 avg_turns=61.80  |  bead vs bead 10-2-8 avg_turns=49.15  |  braid vs bead 8-2-10 avg_turns=60.60  |  bead vs braid 15-0-5 avg_turns=45.55
# elapsed_sec=627.3
```

Unweighted mean of the ten pairing `avg_turns` columns: **53.03**.

`# end_reasons` still prints `low_value_wins` / `low_value_draws` for the
material-value fallback (and full-tie draws) because those endings
carry a `LowValueResolution` payload. The `EndReason` itself is
`material_value` (`low_value=0`). Use `# material_value_tiers` for the
three-tier split.

JSONL win/draw split ([`ends_breakdown.txt`](ends_breakdown.txt)):
elim 0-0, locked_out 8-0, control 20-0, material_value 67-2,
turn_limit 0-3; center_hold / petal_majority / insufficient_tiles /
low_value / double_pass all 0.

Material-value tiers from the same JSONL: total_material 49,
board_tiles 14, low_value fallback 4, draw 2.

| Matchup | P1 W-D-L | avg_turns |
|---|---|---|
| mixed vs mixed | 8-1-11 | 48.05 |
| braid vs braid | 10-0-10 | 61.80 |
| bead vs bead | 10-2-8 | 49.15 |
| braid vs bead | 8-2-10 | 60.60 |
| bead vs braid | 15-0-5 | 45.55 |

## vs local-claim (PR #73) and PR #70 cascade

Same budget, pairings, matchups, seed_base=1. Local-claim and PR #70
use the previous immediate `low-value` endgame.

| | material-value | local-claim | PR #70 cascade |
|---|---|---|---|
| P1 W-D-L | 51-5-44 | 40-6-54 | 41-6-53 |
| elim / lock / control / MV or LV / TL | 0 / 8 / 20 / MV 69 / 3 | 0 / 17 / 18 / LV 61 / 4 | 1 / 23 / 20 / LV 51 / 5 |
| opp captures /game | 5.52 (552) | 6.21 (621) | 6.40 (640) |
| self captures /game | 7.03 (703) | 7.58 (758) | 7.24 (724) |
| claims (own / opp) | 2 (0 / 2) | 2 (0 / 2) | n/a (cascade) |
| mm_avg / mm_max | 3.19 / 16 | 3.36 / 16 | 3.37 / 16 |
| mcts_avg / mcts_max | 1.72 / 7 | 1.74 / 7 | 1.73 / 6 |
| mean pairing avg_turns | 53.03 | 56.40 | 59.87 |
| elapsed_sec | 627.3 | 750.1 | 756.8 |

n=100, not a lock. First-player rate flipped versus local-claim
(P1 **51-5-44** vs **40-6-54**). Most both-below-5 endings now stop on
total material (49) rather than the lowest piece; lockouts dropped
17 → 8 relative to local-claim. Mean pairing length is shorter
(**53.03** vs **56.40** / **59.87**).
