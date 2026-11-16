# Bead scoring: independent vs shared channel

Compares two bead scoring rules on the same search, pick, and deck package.
Doubles allowed, zags on. Do not overwrite; quote numbers from the committed
logs in this directory.

## Rules

Both rules keep the same **matching** test: a bead of color `C` is matched iff
the neighbor has `C` among non-zag rings (presence, not coverage). A match is
still −1 at most once per edge. Zag and braid channels are unchanged.

| | Combine | 1 unmatched bead vs 2-ring hitch | 3 unmatched beads vs 1-ring |
|---|---|---|---|
| **A. Independent** (default, `--bead-scoring independent`) | normal + bead | hitch 1 + unmatched 1 → `(0, 2)` | hitch 1 + unmatched 3 → `(0, 4)` |
| **B. Shared channel** (`--bead-scoring shared`) | `max(normal, bead)` per match and per mismatch | `max(1, 1)` → `(0, 1)` | `max(1, 3)` → `(0, 3)` |

Rule B stops beads doubling with a normal hitch. Beads still count per unmatched
ring when that count exceeds the hitch, so they remain a bomb against
non-matching braids (failed-braid +1 still stacks).

## Deck archetypes

Collections are 27 tiles; the 7+4 bench still uses `--pick search` (heuristic
rank + hill-climb + short rollouts under the same work budget). Generation is
the usual ring/color/zag/braid/bead process with biased numerators, then a
quota retry so the picker actually sees the intended tiles. Zag chance stays
at 1/7 (zags on).

| Flag | Sampling | Quota on the 27-tile collection |
|---|---|---|
| `mixed` | `braid_numer=1`, `bead_numer=1` (defaults) | none — the usual noisy deal |
| `braid` | `braid_numer=5`, `bead_numer=0` | ≥10 braid rings and ≥2 multi-braid (2–3 slot) tiles. Leftover slots after a 2-braid stay normal → braid+normal packing. |
| `bead` | `bead_numer=5`, `braid_numer=0` | ≥10 bead rings, ≥2 single-bead tiles, ≥1 tile with 2–3 beads. |

When the two sides have different archetypes the collections are generated
independently even if `identical_bags` is on. Same-archetype matchups still
mirror (then pick with knowledge of the opponent's 7).

Matchups in this package: `mixed vs mixed`, `braid vs braid`, `bead vs bead`,
`braid vs bead`, `bead vs braid` (the last two are seat-swapped mirrors of
each other).

## Package

Release `apa-kan-bench`, `--pick search`, `--jobs 4`, `--no-depth-probe`,
`--no-same-piece-twice off`, `--pairings minimax:mcts,mcts:minimax`.

| File | Rule | Budget | Games |
|---|---|---|---|
| `independent_shallow` | A | `nodes:132000,iters:2600` (calibrated) | 50 = 5 matchups × 2 seats × 5 seeds |
| `independent_deep` | A | `nodes:528000,iters:10400` (4× calibrated) | 10 = 5 matchups × 2 seats × 1 seed |
| `shared_shallow` | B | calibrated | 50 |
| `shared_deep` | B | 4× calibrated | 10 |

Deep eval is for **balance-breaking strategies**: an archetype, opening, or
line that gives one side an unbeatable or near-forced win regardless of seat
or opponent. Secondary: single-bead captures of a fresh 2-ring, stalls.

`# archetype_seat` in the text log is P1/P2 W-D-L for each archetype across
every game that archetype played. A dominant braid (or bead) strategy shows
up as a strong record from **both** seats, especially in the cross matchup.
`# short_games` counts wins in ≤20 turns (lockout/control) as a forced-win
hint; inspect those JSONL records for counterplay.
