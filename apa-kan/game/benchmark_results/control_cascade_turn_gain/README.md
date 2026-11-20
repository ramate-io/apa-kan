# Control-cascade turn-gain (shallow H2H)

Stacked on PR #74 (`control-cascade` one-way ratchet + immediate unique-holder
drop wins, local-claim, independent-max). `--cascade-turn-gain on` awards one
extra turn to the strict material leader when a checkpoint drops and nobody
wins immediately. Quote numbers from the committed `*.txt` log in this
directory.

PR #74's committed [`control_cascade/h2h_shallow.txt`](../control_cascade/h2h_shallow.txt)
is the previous **live-target** log (`max_mat.min(5)`), not a re-run of the
ratchet. The ratchet re-run is [`control_cascade/h2h_shallow_ratchet.txt`](../control_cascade/h2h_shallow_ratchet.txt)
(brought in by forward-merging `e725097`).

## Package

Release `apa-kan-bench`, `--budget nodes:33000,iters:650` (exactly one
quarter of calibrated `nodes:132000,iters:2600`), `--pick search`,
`--jobs 4`, `--no-depth-probe`, `--no-same-piece-twice off`, `--zags on`,
`--zag-mismatch sum`, `--channel-mode independent-max`, `--endgame-rule
control-cascade`, `--cascade-turn-gain on`, `--capture-mode local-claim`,
`--pairings minimax:mcts,mcts:minimax`, `--deck-matchups
mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`, `--games 10`
(100 games). Replay: [`run_h2h_shallow.sh`](run_h2h_shallow.sh).

| File | Games | Budget |
|---|---|---|
| [`h2h_shallow.txt`](h2h_shallow.txt) | 100 | nodes:33000,iters:650 |

Per-game JSONL: [`h2h_shallow.jsonl.gz`](h2h_shallow.jsonl.gz).

From [`h2h_shallow.txt`](h2h_shallow.txt):

```
# rules: ... control_hold_turns=1 endgame_rule=control-cascade cascade_turn_gain=on ... channel_mode=independent-max capture_mode=local-claim ... zag_mismatch=sum ...
# seat_totals  P1 47-3-50  P2 50-3-47  games=100
# end_reasons  elim=3 locked_out=20 center_hold=0 petal_maj=0 control=73 insuff=0 low_value=1 low_value_wins=1 low_value_draws=0 turn_limit=3 double_pass=0
# control_cascade  target5=21 target4=13 target3=20 target2=19  immediate=29 hold=44  low_value_max_mat_le1=1
# turn_gain  awarded=71 avg_per_game=0.71  mover=63 opponent=8  played=70  ended_on=17 ended_after=6 ended_on_or_after=23
# claims  total=7 avg_per_game=0.07  own=4 opponent=3  healed=3 healed_avg_per_game=0.03
# search_depth  mm_avg=3.18 mm_max=16 mm_n=3902  mcts_avg=1.73 mcts_max=7 mcts_n=3889
# matchup_totals  mixed vs mixed 9-0-11 avg_turns=59.60  |  braid vs braid 7-3-10 avg_turns=123.70  |  bead vs bead 10-0-10 avg_turns=64.80  |  braid vs bead 9-0-11 avg_turns=65.65  |  bead vs braid 12-0-8 avg_turns=74.20
```

Unweighted mean of the ten pairing `avg_turns` columns: **77.59**.

| Matchup | P1 W-D-L | avg_turns |
|---|---|---|
| mixed vs mixed | 9-0-11 | 59.60 |
| braid vs braid | 7-3-10 | 123.70 |
| bead vs bead | 10-0-10 | 64.80 |
| braid vs bead | 9-0-11 | 65.65 |
| bead vs braid | 12-0-8 | 74.20 |

## Versus PR #74 shallow (live-target log)

Quoted from [`control_cascade/h2h_shallow.txt`](../control_cascade/h2h_shallow.txt)
(pre-ratchet live target; ratchet was not re-run there):
P1 **49-12-39**, control=62 (19/13/19/11 at targets 5/4/3/2), claims=1,
mean avg_turns **87.69**, `mm_avg=3.42 mm_max=16`, `mcts_avg=1.74 mcts_max=7`.
No `# turn_gain` line and no immediate/hold split.

This log: P1 **47-3-50**, control=73 (21/13/20/19 at 5/4/3/2; immediate=29
hold=44), claims=7, mean avg_turns **77.59**, `mm_avg=3.18 mm_max=16`,
`mcts_avg=1.73 mcts_max=7`. Turn-gain awarded 71 (0.71/game; mover 63,
opponent 8); 23 games ended on or right after an extra turn.
