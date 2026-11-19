# Core-pass generation H2H

Stacked on PR #68 (`cursor/character-asset-library-b14f`). **Do not merge.**
`sample_tile_with` now runs `CorePass` (rfc-000-000-00n) instead of the
independent ring / zag / braid / bead / emblem RNGs. Live mixed still uses
emblem weights **200,1,1,1,1**; collectors still overlay CLI numerators.

Quote numbers from the committed logs in this directory. Do not overwrite;
append a new file if a rerun is needed.

## Package

Release `apa-kan-bench`, zags on, doubles allowed (`SamePieceTwice::Off`),
shared-channel beads with 2–3 groups (live default), `--pick search`,
`--jobs 4`, `--no-depth-probe`. Never doubles-disallowed.

Both seats `--search-profile deep_mcts`. Work budgets: 4×
`nodes:528000,iters:10400` and long `nodes:132000,iters:10400`.

Deck matchups: `mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`.

## Logs

| File | What |
|---|---|
| `h2h_4x.txt` / `h2h_4x.jsonl.gz` | 5 games at 4× node budget |
| `mcts_long.txt` / `mcts_long.jsonl.gz` | 30 games, 4× iters |

## Results

Numbers below are copied from the committed files in this directory.
`--search-profile deep_mcts` on both seats. Elapsed: `h2h_4x.txt`
**1438.4 s**, `mcts_long.txt` **2700.6 s**.

Default generation (`# ring_process`, n=20000, weights 200,1,1,1,1):

| Line | n1 / n2 / n3 | zags | braid pieces | bead groups | maroon rings |
|---|---|---|---|---|---|
| `# ring_process` | **11098 / 6647 / 2255** | | | | |
| `# ring_process_zags` | rings=31157 | **4913** (rate **0.1577**) | | | |
| `# ring_process_braids` | | | **540** (piece_rate **0.0270**) | | |
| `# ring_process_beads` | | | | **249** (piece_rate **0.0124**) | |
| `# ring_process_colors` | | | | | **353** |

`# ring_process_emblems` pieces=20000 weights=200,1,1,1,1: crescent **103**
(rate **0.0052**), zs **99** (**0.0050**), blocker **105** (**0.0053**),
rosette **94** (**0.0047**).

Branching (`# sample_games=12`): ALL n=569 avg **44.58** max **92**.

### 4× (`h2h_4x.txt`)

`nodes:528000,iters:10400`, 1 pairing × 5 matchups × 1 game.

```
# seat_totals  P1 3-0-2  P2 2-0-3  games=5
# end_reasons  elim=0 locked_out=1 center_hold=0 petal_maj=0 control=4 insuff=0 low_value=0
# emblems  crescent_saves=0 avg_per_game=0.00  zs_bonuses=0 avg_per_game=0.00
# search_depth  mm_avg=0.00 mm_max=0 mm_n=0  mcts_avg=3.37 mcts_max=19 mcts_n=245
# root_spread  entropy=1.34 eff_branch=6.1 top1=0.62 near_q=7.0 q_spread10=0.332 stability=0.83 legal=36.2 n=245
# captures  opponent_tiles_removed=10  avg_per_game=2.00  self_captures=15  self_avg_per_game=3.00
```

| Matchup | W-D-L | avg_turns |
|---|---|---|
| mixed vs mixed | **1-0-0** | **19.00** |
| braid vs braid | **0-0-1** | **28.00** |
| bead vs bead | **0-0-1** | **22.00** |
| braid vs bead | **1-0-0** | **127.00** |
| bead vs braid | **1-0-0** | **49.00** |

### Long (`mcts_long.txt`)

`nodes:132000,iters:10400`, 1 pairing × 5 matchups × 6 games.

```
# seat_totals  P1 17-0-13  P2 13-0-17  games=30
# end_reasons  elim=0 locked_out=2 center_hold=0 petal_maj=0 control=24 insuff=0 low_value=4
# emblems  crescent_saves=0 avg_per_game=0.00  zs_bonuses=1 avg_per_game=0.03
# search_depth  mm_avg=0.00 mm_max=0 mm_n=0  mcts_avg=3.02 mcts_max=19 mcts_n=1335
# root_spread  entropy=1.59 eff_branch=9.5 top1=0.57 near_q=11.3 q_spread10=0.234 stability=0.83 legal=39.8 n=1335
# captures  opponent_tiles_removed=72  avg_per_game=2.40  self_captures=85  self_avg_per_game=2.83
# braid_edges  occupied_with_braid=1803 satisfied=848 rate=0.4703
```

| Matchup | W-D-L | avg_turns |
|---|---|---|
| mixed vs mixed | **5-0-1** | **26.50** |
| braid vs braid | **1-0-5** | **37.17** |
| bead vs bead | **2-0-4** | **34.67** |
| braid vs bead | **5-0-1** | **66.17** |
| bead vs braid | **4-0-2** | **58.00** |
