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

Both seats `--search-profile deep_mcts` (minimax uses the enhanced ID knobs
from that profile). Work budgets: calibrated `nodes:132000,iters:2600` and
4× `nodes:528000,iters:10400` (MCTS long uses `iters:10400` only).

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

Numbers below are copied from the committed files in this directory.
`--search-profile deep_mcts` on both seats (minimax uses the enhanced ID
knobs from that profile). Elapsed: `h2h.txt` **972.3 s**, `h2h_4x.txt`
**736.1 s**, `mcts_long.txt` **2665.6 s**.

Default generation (`# ring_process_emblems`, weights 200,1,1, n=20000):
crescent **106** (rate **0.0053**), zs **101** (rate **0.0050**).

### Ability triggers (`# emblems`)

| Log | Games | crescent_saves (avg/game) | zs_bonuses (avg/game) |
|---|---|---|---|
| `h2h.txt` | 48 | **115** (**2.40**) | **152** (**3.17**) |
| `h2h_4x.txt` | 10 | **13** (**1.30**) | **7** (**0.70**) |
| `mcts_long.txt` | 30 | **64** (**2.13**) | **142** (**4.73**) |

Crescent save = live tiles that would fail `eff >= weight` but pass the
one-adjacency test. Zs bonus = +1 applications on mismatching incident
edges of Two Zag Zs tiles. Counted after the ADD/MOVE lands, before
removals.

### Calibrated 48 (`h2h.txt`)

`nodes:132000,iters:2600`, 2 pairings × 24 directed matchups × 1 game.

```
# seat_totals  P1 22-0-26  P2 26-0-22  games=48
# end_reasons  elim=0 locked_out=24 center_hold=0 petal_maj=0 control=10 insuff=0 low_value=14
# search_depth  mm_avg=4.27 mm_max=16  mcts_avg=2.23 mcts_max=10
# short_games  turns_le_20=2 lockout=0 control=0
# higher_opening_strength_won=35/47
```

| Deck | P1 W-D-L | P2 W-D-L | Combined |
|---|---|---|---|
| mixed | 4-0-2 | 3-0-3 | 7-0-5 |
| braid | 1-0-5 | 1-0-5 | 2-0-10 |
| bead | 0-0-6 | 1-0-5 | 1-0-11 |
| crescent | 7-0-3 | 8-0-2 | **15-0-5** |
| zs | 5-0-5 | 7-0-3 | 12-0-8 |
| emblem | 5-0-5 | 6-0-4 | 11-0-9 |

Directed matchup totals (P1 view, 2 games each):

| Matchup | P1 W-D-L | avg turns |
|---|---|---|
| crescent vs mixed | 1-0-1 | 40.50 |
| mixed vs crescent | 1-0-1 | 33.50 |
| crescent vs braid | 2-0-0 | 47.00 |
| braid vs crescent | 0-0-2 | 37.00 |
| crescent vs bead | 2-0-0 | 30.00 |
| bead vs crescent | 0-0-2 | 39.00 |
| zs vs mixed | 1-0-1 | 37.50 |
| mixed vs zs | 1-0-1 | 24.50 |
| zs vs braid | 1-0-1 | 31.50 |
| braid vs zs | 0-0-2 | 33.00 |
| zs vs bead | 2-0-0 | 31.00 |
| bead vs zs | 0-0-2 | 38.00 |
| emblem vs mixed | 1-0-1 | 28.50 |
| mixed vs emblem | 2-0-0 | 48.00 |
| emblem vs braid | 2-0-0 | 39.00 |
| braid vs emblem | 1-0-1 | 71.50 |
| emblem vs bead | 1-0-1 | 59.50 |
| bead vs emblem | 0-0-2 | 47.00 |
| crescent vs zs | 1-0-1 | 21.50 |
| zs vs crescent | 1-0-1 | 28.50 |
| crescent vs emblem | 1-0-1 | 121.50 |
| emblem vs crescent | 0-0-2 | 51.00 |
| zs vs emblem | 0-0-2 | 26.00 |
| emblem vs zs | 1-0-1 | 57.50 |

Collector vs control, both seats (from those rows): mixed **5-0-7**, braid
**10-0-2**, bead **11-0-1**. Collector vs collector: crescent vs zs **2-0-2**,
crescent vs emblem **3-0-1**, emblem vs zs **3-0-1**.

### 4× 10 (`h2h_4x.txt`)

`nodes:528000,iters:10400`, 2 pairings × 5 matchups × 1 game.

```
# seat_totals  P1 8-0-2  P2 2-0-8  games=10
# end_reasons  elim=0 locked_out=1 control=0 low_value=9
# search_depth  mm_avg=4.92 mm_max=16  mcts_avg=2.78 mcts_max=12
# short_games  turns_le_20=2 lockout=0 control=0
```

| Matchup | P1 W-D-L | avg turns |
|---|---|---|
| crescent vs mixed | 1-0-1 | 27.50 |
| mixed vs crescent | 2-0-0 | 44.00 |
| zs vs bead | 1-0-1 | 29.50 |
| bead vs zs | 2-0-0 | 24.00 |
| crescent vs zs | 2-0-0 | 19.00 |

Both seats: mixed vs crescent **3-0-1**, bead vs zs **3-0-1**. Crescent as
P1 vs zs **2-0-0** (zs as P1 not in this batch).

### MCTS long 30 (`mcts_long.txt`)

`--pairings mcts:mcts`, `--games 6`, `nodes:132000,iters:10400`. Same shape
as `deep_mcts/mcts_long`.

```
# seat_totals  P1 18-1-11  P2 11-1-18  games=30
# end_reasons  elim=0 locked_out=3 control=4 low_value=23 (wins 22, draws 1)
# search_depth  mcts_avg=2.82 mcts_max=19
# short_games  turns_le_20=3 lockout=0 control=0
```

| Matchup | P1 W-D-L | avg turns |
|---|---|---|
| mixed vs mixed | 2-0-4 | 47.33 |
| crescent vs crescent | 4-0-2 | 33.33 |
| zs vs zs | 3-0-3 | 33.83 |
| emblem vs emblem | 3-1-2 | 33.50 |
| crescent vs zs | **6-0-0** | 54.33 |

`crescent vs zs` games: 23, 17, 31, 87, 145, 23 turns (one lockout at 145).
Not a four-move line. zs as P1 vs crescent was not in this batch.

### +1 cost

Stuffed collectors open much heavier than mixed/braid/bead
(`opening_strength_equal=0/48` in `h2h.txt`; `higher_opening_strength_won=35/47`).
That is picker + emblem surcharge, not the ability alone.

Against mixed (rare emblems, same ring process) collectors are **5-0-7**
at the calibrated budget. At 4×, mixed vs crescent is **3-0-1**. +1 cost
is **not clearly underpriced** in that comparison.

Against braid/bead, stuffed collectors are **10-0-2** / **11-0-1**
calibrated, then **bead vs zs 3-0-1** at 4×. Those blowouts track opening
weight more than a rules exploit.

Crescent is the strongest stuffed deck in these logs (calibrated **15-0-5**;
P1 vs zs **2-0-0** at 4× and **6-0-0** in `mcts_long`). Combined crescent
as P1 vs zs across the three files is **9-0-1**. zs as P1 vs crescent is
only the calibrated **1-0-1**. Flag as a seat+deck lean, not an unbeatable
script: short games are 2 / 2 / 3 across the three logs, all with
`lockout=0`, and the long-batch sweep includes a 145-turn lockout.

No repeated four-move winning line in these files.

## Open design questions

See [`rfc-001-001-006-emblems`](../../../../rfc/rfc-001-001-006-emblems/README.md).
Short list (interpretations unchanged; not decided):

1. Crescent adjacency = unique occupied neighbors (implemented), not incident-arc count.
2. Crescent save is `eff == n_rings` at one adjacency (`eff > n_rings` still overwhelms).
3. Two Zag Zs +1 is per mismatching incident edge on the emblem tile, not once per tile and not mirrored to the neighbor.
4. Inner↔center can apply the Zs bonus twice (two arcs).
5. Whether +1 deck cost is a fair price — see **+1 cost** above.
