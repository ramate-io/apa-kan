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

Logs and tables are filled in as each run commits.
