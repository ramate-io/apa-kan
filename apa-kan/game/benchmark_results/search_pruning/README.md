# Search pruning / deeper play

Stacked on PR #62 (`cursor/bead-shared-channel-1647`). Makes minimax and MCTS
search deeper inside the **same** work budgets, then reruns the #62 archetype
matchups with the stronger engines.

Quote numbers from the committed logs in this directory. Do not overwrite;
append a new file if a rerun is needed.

## Profiles

| Profile | Minimax | MCTS |
|---|---|---|
| `plain` | alpha-beta, no TT/ordering/LMR/PVS/qsearch | uniform random cut-off playouts, no PW |
| `classic` | PR #62: TT + ordering + quiescence | heuristic cut-off playouts, no PW, 1 thread |
| `enhanced` (default) | classic + LMR (fail-high re-search, skip root) + PVS | classic + progressive widening |

`--search k=on\|off` overlays flags. `--search-threads N` runs N independent
MCTS trees at the root; worker `t` is seeded with `rng.gen() XOR (t+1)*golden`.
`threads=1` draws nothing extra and is bit-identical. Keep `threads=1` with
`--jobs` so game-level parallelism stays deterministic.

`--ablate-search` writes one row per lever × opening/midgame position.
`--ply-probe N` times a handful of positions against the work/time limit and
reports completed ID depth vs the target.

## Package

Release `apa-kan-bench`, zags on, doubles allowed, shared-channel beads with
2–3 groups (live default), `--pick search`, `--jobs 4`, `--no-depth-probe`,
`--pairings minimax:mcts,mcts:minimax`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`.

`# search_depth` is minimax completed iterative-deepening depth (avg/max,
move-weighted) and MCTS visit-weighted tree depth excluding the root.

## 10-ply probe (committed)

`--ply-probe 10 --p1-deck braid --p2-deck bead`, enhanced unless noted.
10 ply was **not reached** on any position. Midgame `braid_vs_bead_ply24` is
24 greedy plies from a braid vs bead opening.

| Log | Limit | open | open2 | braid_vs_bead_ply24 |
|---|---|---|---|---|
| `ply10_calibrated.txt` | 132000 nodes | 3 / 1951 ms | 4 / 2109 ms | 4 / 867 ms |
| `ply10_4x.txt` | 528000 nodes | 4 / 7134 ms | 4 / 4353 ms | 5 / 4007 ms |
| `ply10_20s.txt` | 20 s wall | 5 / 1.62M nodes | 6 / 2.02M nodes | 6 / 3.31M nodes |
| `ply10_20s_noq.txt` | 20 s, qsearch off | 6 / 6.09M nodes | 6 / 5.80M nodes | 8 / 4.64M nodes |

At ~55 legal moves, 10-ply ID is not feasible at the calibrated budget.
20 s without quiescence reached 8 ply on the narrower midgame (54 legal).

## Full-game package

Same pairings/matchups as #62. Quote `# search_depth` and W-D-L from the
log files once they are committed.

| File | Budget | Games |
|---|---|---|
| `ablate.txt` | 132000 / 2600 | 4 positions × 10 profiles |
| `enhanced_shallow` | 132000 / 2600 | 50 |
| `enhanced_deep` | 528000 / 10400 | 10 |
| `h2h_*` | 132000 / 2600 | 6 mixed games per seat × engine |
| `ply_target_braid_bead` | 2000000 / 5200 | 4 braid/bead |
