# Shared-channel default with 2–3 bead groups

Rerun of the [`../`](../) package under the **live default**: rule B
(shared-channel bead scoring) and piece-level **2–3 bead groups** (no
single-bead tiles). Doubles allowed, zags on. Do not overwrite; quote numbers
from the committed logs in this directory.

Compare against the previous rule-B runs in [`../shared_shallow`](../shared_shallow.txt)
and [`../shared_deep`](../shared_deep.txt), which still generated 1-bead tiles.

## Package

Release `apa-kan-bench`, `--pick search`, `--jobs 4`, `--no-depth-probe`,
`--no-same-piece-twice off`, `--pairings minimax:mcts,mcts:minimax`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`.
Bead scoring is the default (`shared`); independent remains
`--bead-scoring independent`.

| File | Budget | Games |
|---|---|---|
| `groups_shallow` | `nodes:132000,iters:2600` (calibrated) | 50 = 5 matchups × 2 seats × 5 seeds |
| `groups_deep` | `nodes:528000,iters:10400` (4× calibrated) | 10 = 5 matchups × 2 seats × 1 seed |

Bead-heavy quota: ≥10 bead rings, ≥2 tiles with 2+ bead rings, ≥1 3-bead
tile, and no 1-bead tiles.

`# search_depth` is minimax completed iterative-deepening depth (avg/max)
and MCTS visit-weighted tree depth excluding the root (avg/max).

Results are filled after the committed logs land.
