# RFC 001-001-006: Emblem slot (Crescent Moon and Two Zag Zs)

## Problem

Capture marks sit at the **bottom** of a tile and change where captures go. There is no matching **top** slot for a rare, costed ability. This RFC adds one optional **emblem** per tile: a generation-weighted glyph that raises deck-building cost by +1 in exchange for a small rules exception.

## Proposed solution

Each tile has **one emblem slot**. The slot is empty on most tiles.

| Emblem | Generation weight | Deck cost | Ability |
|---|---|---|---|
| **None** | 200 | 0 | Full rings at the top of the face. |
| **Crescent Moon** | 1 | +1 | When the piece has exactly one occupied neighbor, mismatches equal to the tile's rings do **not** overwhelm it. Strictly greater still overwhelms. |
| **Two Zag Zs** | 1 | +1 | Each incident edge that already has a zag mismatch scores **+1 more** on this tile's zag channel. |

CLI: `--emblem-weights none,crescent,zs` (default `200,1,1`). `--emblems off` is `1,0,0` and **skips the emblem RNG** so the same seed deals the pre-emblem rings/dots/marks. `--emblems on` restores the default.

### Cost vs overwhelm

`Tile::cost` is `n_rings + emblem.cost()` (surcharge [`EMBLEM_COST`] = 1). That is the deck-building statistic used by the 7+4 picker, eval, and low-value **SECONDARY** weights.

`Tile::weight` stays `n_rings`. Overwhelm still compares against `weight` (the ring-slot count). Putting the +1 into `weight` would make Crescent Moon a no-op on `mismatch == rings`, which is the case the ability is written for.

### Crescent Moon

Live capture threshold is `effective >= weight` (`weight == n_rings`). Crescent Moon with **exactly one occupied neighbor** uses `effective > n_rings` instead.

- Occupied neighbors are **unique nodes**, not incident generating-circle arcs. Inner↔center is two petals / two arcs but one neighbor, so it counts as one adjacency (same occupancy idea as petal majority).
- `effective` still walks **incident arcs** (inner↔center can contribute twice).
- `mismatch == rings` with one adjacency: **saved** (not captured).
- `mismatch > rings` with one adjacency: **overwhelmed**.
- Two or more occupied neighbors: ordinary `eff >= weight`.

Trigger counter `crescent_saves`: live tiles that would fail `eff >= weight` but pass the one-adjacency test. Counted on the board after the ADD/MOVE lands and **before** removals.

### Two Zag Zs

Applied in `Game::edge_channels` after the ordinary zag-mismatch count, on the tile being scored:

```
if tile.emblem is TwoZagZs and zag_mismatch > 0 { zag_mismatch += 1 }
```

- **+1 per incident edge** whose pre-bonus zag mismatch is already > 0, not once per tile.
- Applied when scoring **this** tile. The neighbor does not receive the bonus unless it also has Two Zag Zs.
- Inner↔center can apply twice (two incident arcs to the same neighbor).
- Shared-channel bead scoring is unchanged: the extra point is on the **zag** channel, which always adds.

Trigger counter `zs_bonuses`: number of those +1 applications on the live board (same timing as crescent saves).

### Generation and picking

`sample_tile_with` rolls the emblem after capture mark. Collector archetypes stuff the weights and retry until a quota:

| Archetype | CLI | Weights | Quota (27-tile collection) |
|---|---|---|---|
| `crescent` | `--p1-deck crescent` | none=1, crescent=20, zs=0 | ≥8 Crescent Moon tiles |
| `zs` | `--p1-deck zs` | none=1, crescent=0, zs=20 | ≥8 Two Zag Zs tiles |
| `emblem` / `emblem-mix` | `--p1-deck emblem` | none=1, crescent=20, zs=20; braid_numer=2, bead_numer=2 | ≥4 of each emblem |

Mixed / braid / bead keep the default 200,1,1 (emblems stay rare). The picker scores emblem cost (+9) plus a small bonus for each glyph so stuffed collections actually draft them.

### Visual

Emblems sit at the **top** of the tile, warped/rotated with the face like capture marks at the bottom.

- Non-empty emblem: rings (normal, zag, braid, bead) open a ~70° **C-notch** at the top, same idea as the capture-mark notch.
- Empty slot: full rings at the top.
- Crescent Moon: a thick C (filled) opening toward the rim.
- Two Zag Zs: two small angular Z strokes.

Desktop PNG close-ups: [`emblem_crescent_dot.png`](../../apa-kan/game/screenshots/emblem_crescent_dot.png), [`emblem_crescent_triangle.png`](../../apa-kan/game/screenshots/emblem_crescent_triangle.png), [`emblem_zs_dot.png`](../../apa-kan/game/screenshots/emblem_zs_dot.png), [`emblem_zs_triangle.png`](../../apa-kan/game/screenshots/emblem_zs_triangle.png).

### Search, logs, TOML

- Search packs `kind | (capture << 16) | (emblem << 17)`. TT `b` is `dots | (capture << 2) | (emblem << 3)`. `KIND_COUNT` is not multiplied (that overflows `u16`).
- Zobrist XORs `board_emblem` / `bench_emblem` tables.
- Eval adds the emblem surcharge on board and bench material.
- JSONL: each tile has `emblem` (`none`/`crescent`/`zs`; absent on older logs = none). Each action has `crescent_saves` / `zs_bonuses`. Game totals are summed. `rules.emblem_weights` is `none,crescent,zs`.
- Scenario TOML: `emblem = "crescent"` / `"zs"` on a listed tile. Omitted = none.

## Implementation

- `Emblem`, `EmblemWeights`, `Tile.emblem`, `Tile::cost`, `RuleSet.emblem_weights` in `apa-kan/game/src/game.rs`
- `--emblem-weights`, `--emblems`; collector archetypes in `deck.rs`
- Crescent / Zs in `overwhelms` / `edge_channels`; `EmblemTrigger` on `ActionOutcome`
- Search/TT/Zobrist extra bits; eval surcharge
- JSONL + bench `# emblems` / `# ring_process_emblems` / `# archetype_seat`
- Top notch + glyphs in `visual.rs` / `ui.rs` / `render.rs`

## Alternatives considered

- Folding +1 cost into `weight`: rejected; Crescent Moon would never save `mismatch == rings`.
- Counting inner↔center as two adjacencies for Crescent: rejected; the user said "one adjacency" in occupancy language. Arc-double scoring still applies to `effective`.
- Mirroring the Zs +1 onto both tiles of the edge: rejected; the glyph is on one tile. Documented as an open question.
- Once-per-tile Zs bonus: rejected in favor of per mismatching incident edge, which matches "if there is a zag mismatch on the tile's edge".

## Open questions

1. **Crescent adjacency.** Unique occupied neighbors (implemented) vs incident-arc count (would make a lone inner-center contact "two adjacencies" and deny the save). Faithful reading of "the piece only has one adjacency" is occupancy.
2. **Crescent threshold.** Live rule is `eff >= weight`. The save is `eff == n_rings` at one adjacency (`eff > n_rings` still overwhelms). If `weight` ever diverges from `n_rings`, which side of the comparison should Crescent use?
3. **Two Zag Zs once vs per edge.** Implemented **per mismatching incident edge** on the emblem tile. A tile with two zag-hitting neighbors gets +2. A once-per-tile reading is also plausible.
4. **Inner-center double petal.** Those two arcs can each take the Zs +1. Should they collapse to one bonus because they share a neighbor?
5. **Is +1 cost a fair price?** Collector H2H in [`emblems`](../../apa-kan/game/benchmark_results/emblems/) is the evidence. Against mixed, stuffed collectors are 5-0-7 calibrated (`h2h.txt`) and mixed vs crescent is 3-0-1 at 4× — not clearly underpriced. Stuffed collectors beat braid/bead at calibrated budget via opening weight (`higher_opening_strength_won=35/47`). Interpretations of questions 1–4 stay as implemented.

## H2H

Release `apa-kan-bench`, zags on, doubles allowed, shared-channel 2–3 bead groups, `--pick search`, `--jobs 4`. Both seats `--search-profile deep_mcts` (minimax uses the enhanced ID knobs from that profile). Logs: [`apa-kan/game/benchmark_results/emblems/`](../../apa-kan/game/benchmark_results/emblems/). Quote numbers from the committed files in that directory.

| Log | Games | P1 W-D-L | crescent_saves | zs_bonuses | elapsed |
|---|---|---|---|---|---|
| `h2h.txt` | 48 | 22-0-26 | 115 (2.40/game) | 152 (3.17/game) | 972.3 s |
| `h2h_4x.txt` | 10 | 8-0-2 | 13 (1.30/game) | 7 (0.70/game) | 736.1 s |
| `mcts_long.txt` | 30 | 18-1-11 | 64 (2.13/game) | 142 (4.73/game) | 2665.6 s |

Calibrated `# archetype_seat` combined: crescent **15-0-5**, zs **12-0-8**, emblem **11-0-9**, mixed **7-0-5**, braid **2-0-10**, bead **1-0-11**. Collectors vs mixed **5-0-7**. `mcts_long` crescent vs zs is **6-0-0** (P1; 17–145 turns). +1 cost is not clearly underpriced vs mixed; stuffed collectors beat braid/bead via opening weight (`higher_opening_strength_won=35/47`). Not a four-move unbeatable line. Full tables in the emblems README.
