# RFC 001-001-001: 8-petal rosette redesign

## Problem

Apa Kan on `main` is a Bevy 0.19.1 hello-world. Earlier PRs (#3–#6) explored a square-grid game with scored-mode banks. This RFC is a **clean start**: a new geometric board and a color-overwhelm ruleset, implemented on a new branch off `main` without modifying those PRs.

## Proposed solution

### Board

An **8-petal rosette**. Eight equal circles, each of radius `R`, centered at distance `R` from the origin at angles `offset + k·45°`. Every circle passes through the origin.

- **Nodes**: the origin plus every pairwise circle intersection, deduped with a relative tolerance (`R · 1e-8`).
- **Edges**: the circular arcs between consecutive nodes along each generating circle (including wrap-around).
- **Spawns**: the 8 nodes on the middle positive-radius ring (`r ≈ √2 R`). They form an independent set.

The board is a data type (`Board`) so other generating-circle shapes can be added later via `Board::from_circles`.

Construction stats for the default rosette (`R = 1`, offset `67.5°` so an outer tip sits on +Y):

| | count |
|---|---|
| nodes | 25 (1 center + 8 inner + 8 spawn + 8 outer) |
| generating circles | 8 (7 nodes each) |
| directed CCW arcs | 56 |
| unique neighbor-pairs | 48 (8 extra arcs are the inner double-edges to the center) |
| spawns | 8, ids `[2, 5, 8, 11, 14, 17, 20, 23]`; none adjacent |

**Neighbor invariant (arc-degree):** every non-center node has exactly **4 incident arcs**. The center has 16 incident arcs and 8 unique neighbors (the inner ring). Inner-ring nodes have 3 unique neighbors because two of their four arcs go to the center along different circles. This matches the overlapping inner petals in the reference drawing.

**8-fold symmetry:** rotation by `k·45°` is a graph automorphism that preserves spawns.

### Pieces

Each tile has:

- **Owner** as a light/dark shade: P1 light, P2 dark (fill).
- **Color**, one of exactly three: green, blue, orange (rings).
- **Weight** `w ∈ {1,2,3}` drawn as 1–3 concentric rings.
- **Dots** `d ∈ {1,2,3}` drawn as 1–3 dots (MOVE length).

Weight and dots are independent of each other and of color.

### Bench and bag (`RuleSet` options)

Default:

- Each player has a **bag** of the 27 unique `(color × weight × dots)` kinds, shuffled with a seed.
- **`identical_bags` (default true):** both players get the same shuffle (new tile ids / owners), for fairness.
- Opening **deal of `starting_bench_size` (default 5)** from bag onto the bench.
- **ADD** places one concrete bench tile onto an empty spawn (not an abstract “any kind” picker).
- On a **MOVE**, each **opponent** tile removed draws one more tile from the mover’s bag onto their bench. ADD captures do not draw. Empty bag → the draw is skipped.
- Remaining bag tiles do **not** keep a player alive.

Options: `starting_bench_size`, `identical_bags`, `bag_one_of_each_kind`, `capture_draw_on_move`.

### Overwhelm (by color)

Evaluated **board-wide, simultaneously**, after every action.

- `matches = own weight + (number of adjacent tiles of the same color, either owner)`
- `mismatches = number of adjacent tiles of a different color, either owner`
- Removed iff `mismatches >= matches`

The designer example: a **double green** next to two oranges has matches 2 / mismatches 2 and is overwhelmed; a green neighbor makes matches 3, so it survives. Adjacent same-color tiles of the opponent still count as matches.

**`mismatch_uses_neighbor_weight` (default false):** if on, each mismatch adds the neighbor’s weight instead of 1.

### Actions

One action per turn by default (`actions_per_turn`, default 1):

1. **ADD** a bench tile to an empty spawn. Legal even if the tile would be immediately overwhelmed; the UI warns.
2. **MOVE** one of your tiles along the graph.
3. **PASS** only when you have no legal ADD or MOVE.

### MOVE path semantics

- Length is **exactly `d` steps**, not “up to `d`”. If no exact-length path exists, that tile cannot move.
- The mover is vacated from its start node for the duration of the path (start is empty but visited).
- A path may **not revisit a node**, including nodes hopped over.
- The destination must be empty.
- **Empty step:** walk along one circle-arc to the next node if it is empty. Cost 1.
- **Hop:** if the next node along a chosen circle and direction is occupied, consider the maximal consecutive occupied run along that same circle in that direction. You may spend **1 step** to skip the whole run and land on the first empty node beyond it, iff every hopped tile has `weight >= mover.weight` (`hop_requires_ge_weight`, default true). Owner does not matter. If any hopped tile is lighter, or the run wraps without an empty landing, the hop is illegal. Hopped nodes are marked visited.

Hops are circle-straight: they do not branch with the graph. Multi-arcs between the center and an inner node are distinct as circle-arcs (rendering the inner petals) but lead to the same neighbor for an empty step.

### Win and draw

- **Win (elimination):** the opponent has **no tiles on the board and an empty bench**. A remaining bag does not save them. If both players are eliminated in the same resolution, the game is a **draw**.
- **Draw:** `turn_limit` completed player-turns (default **200**), or a **double pass**.

`turn_count` increments when a player’s action allotment ends (or they pass / have nothing left).

### Play

CLI (unknown args error out with usage):

```
apa-kan --bot1 <human|random|greedy|minimax|mcts> --bot2 <...>
        --seed N --think-ms N --delay-ms N --actions N --screenshot PATH
```

Defaults: P1 human, P2 greedy, seed 1, think 80 ms.

Bots: uniform random; greedy one-ply; minimax with alpha-beta, iterative deepening, and a material / at-risk / mobility heuristic; MCTS (UCT).

Headless round-robin: `apa-kan-bench`.

## Implementation

| Module | Role |
|---|---|
| `board` | Geometric graph |
| `game` | RuleSet, tiles, actions, overwhelm, win |
| `ai` | Bots |
| `ui` | Bevy 0.19.1 table |
| `render` | Headless PNG |
| `config` | Fail-closed CLI |
| `src/bin/bench.rs` | Round-robin |

## Alternatives considered

- Unique-neighbor degree 4 for every non-center node — false for the inner ring; arc-degree 4 is the geometric fact.
- Infinite 27-kind picker with an ADD counter — rejected; benches are concrete tiles dealt from a bag.
- MOVE “up to d” — rejected; exact `d`.
- Overwhelm by owner — rejected; color, as in the original game.

## Open questions

None blocking this RFC. Possible later knobs already on `RuleSet`: mismatch-by-weight, actions-per-turn, non-identical bags.
