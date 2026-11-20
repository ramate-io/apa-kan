# Control-cascade endgame (shallow H2H)

Stacked on local-claim + independent-max. Two packages live here:

- [`h2h_shallow.txt`](h2h_shallow.txt) — previous **live-target** log
  (`max_mat.min(5)`). Do not overwrite.
- [`h2h_shallow_ratchet.txt`](h2h_shallow_ratchet.txt) — current
  **one-way ratchet** 5→4→3→2 (`target.min(max_mat)` while `max_mat >= 2`;
  recaptures never raise it) plus an immediate unique-holder win when the
  ratchet drops, then low-value at `max_mat <= 1`. Quote numbers from the
  committed `*.txt` log you mean.

Same search budget, matchups, and pairings as PR #70
[`independent_max/h2h_shallow.txt`](../independent_max/h2h_shallow.txt)
and PR #73 [`local_claim/h2h_shallow.txt`](../local_claim/h2h_shallow.txt).
`--capture-mode local-claim`. Sibling endgame PR #76 is material-value
(both-below-5 hierarchy), not cascade.

## Package

Release `apa-kan-bench`, `--budget nodes:33000,iters:650` (exactly one
quarter of calibrated `nodes:132000,iters:2600`), `--pick search`,
`--jobs 4`, `--no-depth-probe`, `--no-same-piece-twice off`, `--zags on`,
`--zag-mismatch sum`, `--channel-mode independent-max`, `--endgame-rule
control-cascade`, `--capture-mode local-claim`, `--pairings
minimax:mcts,mcts:minimax`, `--deck-matchups
mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`, `--games 10`
(100 games).

| File | Interpretation | Games | Budget |
|---|---|---|---|
| [`h2h_shallow.txt`](h2h_shallow.txt) | live-target (pre-ratchet) | 100 | nodes:33000,iters:650 |
| [`h2h_shallow_ratchet.txt`](h2h_shallow_ratchet.txt) | one-way ratchet + immediate drop | 100 | nodes:33000,iters:650 |

Per-game JSONL: [`h2h_shallow.jsonl.gz`](h2h_shallow.jsonl.gz) (live-target),
[`h2h_shallow_ratchet.jsonl.gz`](h2h_shallow_ratchet.jsonl.gz) (ratchet).
Ratchet JSONL win/draw split: [`ends_breakdown_ratchet.txt`](ends_breakdown_ratchet.txt).

## Ratchet (quoted)

From [`h2h_shallow_ratchet.txt`](h2h_shallow_ratchet.txt):

```
# rules: ... control_hold_turns=1 endgame_rule=control-cascade ... channel_mode=independent-max capture_mode=local-claim ... zag_mismatch=sum ...
# seat_totals  P1 50-4-46  P2 46-4-50  games=100
# end_reasons  elim=7 locked_out=26 center_hold=0 petal_maj=0 control=62 insuff=0 low_value=2 low_value_wins=1 low_value_draws=1 turn_limit=3 double_pass=0
# control_cascade  target5=21 target4=10 target3=17 target2=14  immediate=22 hold=40  low_value_max_mat_le1=2
# claims  total=2 avg_per_game=0.02  own=0 opponent=2  healed=4 healed_avg_per_game=0.04
# search_depth  mm_avg=3.34 mm_max=16 mm_n=4150  mcts_avg=1.76 mcts_max=7 mcts_n=4141
# matchup_totals  mixed vs mixed 8-0-12 avg_turns=78.45  |  braid vs braid 9-3-8 avg_turns=95.40  |  bead vs bead 13-0-7 avg_turns=67.95  |  braid vs bead 7-1-12 avg_turns=89.55  |  bead vs braid 13-0-7 avg_turns=81.75
# elapsed_sec=1114.0
```

Unweighted mean of the ten pairing `avg_turns` columns: **82.62**.

| Matchup | P1 W-D-L | avg_turns |
|---|---|---|
| mixed vs mixed | 8-0-12 | 78.45 |
| braid vs braid | 9-3-8 | 95.40 |
| bead vs bead | 13-0-7 | 67.95 |
| braid vs bead | 7-1-12 | 89.55 |
| bead vs braid | 13-0-7 | 81.75 |

JSONL ([`ends_breakdown_ratchet.txt`](ends_breakdown_ratchet.txt)):
elim 7-0, locked_out 26-0, control 62-0 (immediate 22, hold 40; targets
21/10/17/14), low_value 1-1, turn_limit 0-3.

## Versus live-target, #73, #76, #70

| | ratchet (this) | live-target `h2h_shallow` | local-claim (#73) | material-value (#76) | PR #70 cascade |
|---|---|---|---|---|---|
| P1 W-D-L | 50-4-46 | 49-12-39 | 40-6-54 | 51-5-44 | 41-6-53 |
| elim / lock / control / LV or MV / TL | 7 / 26 / 62 / LV 2 / 3 | 2 / 24 / 62 / LV 0 / 12 | 0 / 17 / 18 / LV 61 / 4 | 0 / 8 / 20 / MV 69 / 3 | 1 / 23 / 20 / LV 51 / 5 |
| control targets 5/4/3/2 | 21/10/17/14 | 19/13/19/11 | n/a (fixed 5) | n/a (fixed 5) | n/a (fixed 5) |
| immediate / hold | 22 / 40 | n/a (live-target) | n/a | n/a | n/a |
| claims (own / opp) | 2 (0 / 2) | 1 (0 / 1) | 2 (0 / 2) | (see #76 log) | n/a (cascade) |
| heals | 4 | 4 | 4 | (see #76 log) | n/a |
| mm_avg / mm_max | 3.34 / 16 | 3.42 / 16 | 3.36 / 16 | 3.19 / 16 | 3.37 / 16 |
| mcts_avg / mcts_max | 1.76 / 7 | 1.74 / 7 | 1.74 / 7 | 1.72 / 7 | 1.73 / 6 |
| mean pairing avg_turns | 82.62 | 87.69 | 56.40 | 53.03 | 59.87 |
| elapsed_sec | 1114.0 | 1113.4 | 750.1 | 627.3 | 756.8 |

n=100, not a lock. Control count is unchanged at 62 versus live-target;
the ratchet converts 22 of those into immediate drop wins and cuts
turn-limit draws 12→3. Search still almost never Claims.
