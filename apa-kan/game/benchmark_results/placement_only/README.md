# Placement-only win condition (independent-max)

Stacked on PR #70 (`cursor/independent-max-channels-6046`). Quote numbers
from the committed logs in this directory. Do not overwrite; append a new
file if a rerun is needed.

`--win-condition placement-only`: ADD (spawn-only, no suicide) or REMOVE an
overwhelmed tile. MOVE off. The player to move loses
(`EndReason::NoLegalPlacement`) when they have no legal ADD, even if a
REMOVE exists.

## Package

Release `apa-kan-bench`, zags on, doubles allowed, independent-max default,
`--pick search`, `--jobs 4`, `--no-depth-probe`,
`--pairings minimax:mcts,mcts:minimax`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`,
`--win-condition placement-only`. Calibrated budget `nodes:132000,iters:2600`.

| File | Games | Elapsed |
|---|---|---|
| `h2h` | 60 = 5 matchups × 2 seats × 6 | 219.8 s |

## Results (`h2h.txt`)

```
# branching factor
ADD       96           45.25  88
MOVE      0            0.00   0
ALL       96           45.25  88

# seat_totals  P1 0-0-60  P2 60-0-0  games=60
# end_reasons  elim=0 locked_out=0 no_legal_placement=60 ...
# placement_only  games=60  turns_avg=8.00 turns_min=8 turns_max=8  no_legal_placement=60 rate=1.00  p1_wins=0 p1_win_rate=0.00
# captures  opponent_tiles_removed=0  avg_per_game=0.00  self_captures=0
# search_depth  mm_avg=11.25 mm_max=16 mm_n=240  mcts_avg=1.56 mcts_max=3 mcts_n=240
# elapsed_sec=219.8
```

Every game is P2 by `NoLegalPlacement` in exactly 8 turns (the 8 spawn
nodes fill, then P1 has no legal ADD). Search never harvests. MOVE
branching is 0, as specified.

This is the spawn-count / cannot-place-loses interaction documented as an
open question in RFC 001 §6b: with eight spawns, even fill always hands P2
the win before a REMOVE can reopen a spawn.
