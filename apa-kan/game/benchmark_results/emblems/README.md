# Emblem slot H2H

Stacked on PR #65 (`cursor/deep-mcts-601f`). **Do not merge.** Collector
decks stuffed with Crescent Moon / Two Zag Zs, plus mixed/braid/bead
controls.

Quote numbers from the committed logs in this directory. Do not overwrite;
append a new file if a rerun is needed.

## Package

Release `apa-kan-bench`, zags on, doubles allowed (`SamePieceTwice::Off`),
shared-channel beads with 2–3 groups (live default), `--pick search`,
`--jobs 4`, `--no-depth-probe`. Never doubles-disallowed.

Minimax uses `--search-profile enhanced`. MCTS uses `--search-profile
deep_mcts`. Work budgets: calibrated `nodes:132000,iters:2600` and 4×
`nodes:528000,iters:10400` (MCTS long uses `iters:10400` only).

Collector archetypes (`--p1-deck` / `--deck-matchups`):

| Name | Weights | Quota |
|---|---|---|
| `crescent` | none=1, crescent=20, zs=0 | ≥8 Crescent Moon |
| `zs` | none=1, crescent=0, zs=20 | ≥8 Two Zag Zs |
| `emblem` | none=1, crescent=20, zs=20; braid 2/7, bead 2/7 | ≥4 of each |

`--emblem-weights` / `--emblems off` override generation. Ability weights
of 0 skip the emblem RNG.

## Logs

| File | What |
|---|---|
| `h2h.txt` / `h2h.jsonl.gz` | ~50 normal-budget games, emblem decks vs mixed/braid/bead and each other, both seats |
| `h2h_4x.txt` / `h2h_4x.jsonl.gz` | 10 games at 4× budget |
| `mcts_long.txt` / `mcts_long.jsonl.gz` | deep_mcts self-play, 4× iters, similar to `deep_mcts/mcts_long` |

`# emblems` on each log is how often Crescent saved a tile and how often
Two Zag Zs applied a +1 (per mismatching incident edge on the emblem
tile). JSONL per-action `crescent_saves` / `zs_bonuses` are the same
counters.

## Results

Filled after the committed logs land. Quote those files; do not guess
win rates.

## Open design questions

See [`rfc-001-001-006-emblems`](../../../../rfc/rfc-001-001-006-emblems/README.md).
Short list:

1. Crescent adjacency = unique occupied neighbors (implemented), not incident-arc count.
2. Crescent save is `eff == n_rings` at one adjacency (`eff > n_rings` still overwhelms).
3. Two Zag Zs +1 is per mismatching incident edge on the emblem tile, not once per tile and not mirrored to the neighbor.
4. Inner↔center can apply the Zs bonus twice (two arcs).
5. Whether +1 deck cost is a fair price — answer from the logs in this directory.
