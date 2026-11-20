# Control-cascade endgame (shallow H2H)

Stacked on local-claim + independent-max. Default `--endgame-rule
control-cascade`: live control target `max_mat.min(5)` of the 9-node set
while `max_mat >= 2`, then low-value at `max_mat <= 1`. Do not overwrite;
quote numbers from the committed `*.txt` log in this directory.

Compared to PR #70 [`independent_max/h2h_shallow.txt`](../independent_max/h2h_shallow.txt)
(fixed 5 of 9, then both-below-5 low-value). Same search budget, matchups,
and pairings. This package also uses `--capture-mode local-claim`.

## Package

Release `apa-kan-bench`, `--budget nodes:33000,iters:650` (exactly one
quarter of calibrated `nodes:132000,iters:2600`), `--pick search`,
`--jobs 4`, `--no-depth-probe`, `--no-same-piece-twice off`, `--zags on`,
`--zag-mismatch sum`, `--channel-mode independent-max`, `--endgame-rule
control-cascade`, `--capture-mode local-claim`, `--pairings
minimax:mcts,mcts:minimax`, `--deck-matchups
mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`, `--games 10`
(100 games).

| File | Games | Budget |
|---|---|---|
| [`h2h_shallow.txt`](h2h_shallow.txt) | 100 | nodes:33000,iters:650 |

Per-game JSONL: [`h2h_shallow.jsonl.gz`](h2h_shallow.jsonl.gz).

From [`h2h_shallow.txt`](h2h_shallow.txt):

```
# rules: ... control_hold_turns=1 endgame_rule=control-cascade ... channel_mode=independent-max capture_mode=local-claim ... zag_mismatch=sum ...
# seat_totals  P1 49-12-39  P2 39-12-49  games=100
# end_reasons  elim=2 locked_out=24 center_hold=0 petal_maj=0 control=62 insuff=0 low_value=0 low_value_wins=0 low_value_draws=0 turn_limit=12 double_pass=0
# control_cascade  target5=19 target4=13 target3=19 target2=11  low_value_max_mat_le1=0
# claims  total=1 avg_per_game=0.01  own=0 opponent=1  healed=4 healed_avg_per_game=0.04
# search_depth  mm_avg=3.42 mm_max=16 mm_n=4386  mcts_avg=1.74 mcts_max=7 mcts_n=4385
# matchup_totals  mixed vs mixed 8-2-10 avg_turns=77.30  |  braid vs braid 10-5-5 avg_turns=125.35  |  bead vs bead 9-2-9 avg_turns=68.50  |  braid vs bead 8-1-11 avg_turns=78.30  |  bead vs braid 14-2-4 avg_turns=89.00
```

Unweighted mean of the ten pairing `avg_turns` columns: **87.69**.

| Matchup | P1 W-D-L | avg_turns |
|---|---|---|
| mixed vs mixed | 8-2-10 | 77.30 |
| braid vs braid | 10-5-5 | 125.35 |
| bead vs bead | 9-2-9 | 68.50 |
| braid vs bead | 8-1-11 | 78.30 |
| bead vs braid | 14-2-4 | 89.00 |

## Versus PR #70 shallow

Quoted from [`independent_max/h2h_shallow.txt`](../independent_max/h2h_shallow.txt):
P1 **41-6-53**, control=20, low_value=51, mean avg_turns **59.87**,
`mm_avg=3.37 mm_max=16`, `mcts_avg=1.73 mcts_max=6`.

This log: P1 **49-12-39**, control=62 (19/13/19/11 at targets 5/4/3/2),
low_value=0, mean avg_turns **87.69**, `mm_avg=3.42 mm_max=16`,
`mcts_avg=1.74 mcts_max=7`. Search almost never Claims at this budget
(`claims total=1`).
