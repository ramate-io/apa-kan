# Placement-only win condition (independent-max)

Stacked on PR #70 (`cursor/independent-max-channels-6046`). Quote numbers
from the committed logs in this directory.

`--win-condition placement-only`: 13-tile public bench (7+6 from the 27),
no spawn-token cap, ADD still spawn-only, MOVE on with the usual dest
rules, no suicide ADD/MOVE. Later-overwhelmed tiles stay. REMOVE of an
overwhelmed tile is allowed but does **not** count as placement. The
player to move loses (`EndReason::NoLegalPlacement`) when they have no
legal ADD and no legal MOVE.

An earlier light H2H on this path (MOVE off, 11-tile bench, one-token
cap) was 60/60 P2 in exactly 8 turns and is superseded by the files
below.

## Package

Release `apa-kan-bench`, zags on, doubles allowed, independent-max default,
`--pick search`, `--jobs 4`, `--no-depth-probe`, `--zag-mismatch sum`,
`--pairings minimax:mcts,mcts:minimax`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`,
`--win-condition placement-only`. Calibrated budget `nodes:132000,iters:2600`.

| File | Games | Elapsed |
|---|---|---|
| `h2h` | 60 = 5 matchups × 2 seats × 6 | 13834.9 s |

## Results (`h2h.txt`)

```
# branching factor
ADD       879          11.15  104
MOVE      1724         16.33  41
ALL       1749         21.84  104

# rules: ... bag_size=13 ... zag_mismatch=sum ... win_condition=placement-only
# seat_totals  P1 5-53-2  P2 2-53-5  games=60
# end_reasons  elim=0 locked_out=0 no_legal_placement=7 ... turn_limit=53 ...
# placement_only  games=60  turns_avg=190.62 turns_min=16 turns_max=200  no_legal_placement=7 rate=0.12  p1_wins=5 p1_win_rate=0.08
# captures  opponent_tiles_removed=327  avg_per_game=5.45  self_captures=209
# search_depth  mm_avg=6.60 mm_max=16 mm_n=5720  mcts_avg=2.51 mcts_max=65 mcts_n=5717
# elapsed_sec=13834.9
```

Game length is 190.62 turns on average (min 16, max 200). Seat split is
P1 5-53-2 / P2 2-53-5. No-legal-placement is 7/60 (rate 0.12); the other
53 games hit the 200-turn limit. Every decisive game is
`NoLegalPlacement` (5 P1, 2 P2). MOVE branching is live (avg 16.33).
The recorded minimax-vs-MCTS game MOVEs off a spawn on turn 5.
