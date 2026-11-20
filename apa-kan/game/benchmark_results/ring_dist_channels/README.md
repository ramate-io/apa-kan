# Ring-count 7:5:4, ~10% braid rings, independent channels, counterplay search

Live generation previously used a sequential 7:1 keep-last vs draw-any gate
(`RING_SAME_WEIGHT=7`, `RING_ANY_WEIGHT=1`), which made singles ~87%+ of
tiles and braid rings ~0.5%. This package measures the new 7:5:4 count mix
and IndependentMax heavy coin 2:1 (target ~10% braid rings), with
independent-channels on vs off, and the default `enhanced` search vs the
new `counterplay` personality.

Do not overwrite committed `h2h_*` logs; quote numbers from those files.

## Package

Release `apa-kan-bench`, `--no-default-features`, `--budget nodes:33000,iters:650`,
`--games 10`, `--jobs 4`, `--no-depth-probe`, `--zags on`, `--zag-mismatch sum`,
`--endgame-rule control-cascade`, `--cascade-turn-gain on`, `--capture-mode local-claim`,
`--no-same-piece-twice off`, `--pick search`,
`--pairings minimax:mcts,mcts:minimax`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`,
`--seed 1`.

| Log | Channels | Search | Games |
|---|---|---|---|
| `h2h_a_additive_enhanced` | additive / `--independent-channels off` | enhanced | 100 |
| `h2h_b_imax_enhanced` | independent-max / on | enhanced | 100 |
| `h2h_a_additive_counterplay` | additive | counterplay | 100 |
| `h2h_b_imax_counterplay` | independent-max | counterplay | 100 |

Main baseline (old sequential 7:1 generation, independent-max, enhanced) is
the already-committed [`../control_cascade_turn_gain/h2h_shallow.txt`](../control_cascade_turn_gain/h2h_shallow.txt)
(same budget and matchups).

Reproduce with [`run_h2h_shallow.sh`](run_h2h_shallow.sh). Center-holder
survey: [`survey_center.py`](survey_center.py).

## Results

Filled after the shallow H2H runs.
