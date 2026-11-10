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
3. **PASS** only when you have no legal ADD or MOVE **and** `no_moves_loses` is off.

### MOVE path semantics

- Length is **exactly `d` steps**, not “up to `d`”. If no exact-length path exists, that tile cannot move.
- The mover is vacated from its start node for the duration of the path (start is empty but visited).
- The destination must be empty and must differ from the start.
- **Path uniqueness** is `path_constraint` (default `NoEdgeRevisit`; see stall-prevention):
  - `NoNodeRevisit` — a path may not revisit a node, including nodes hopped over (legacy).
  - `NoEdgeRevisit` — a path may not traverse the same undirected circle-arc twice. Nodes may be revisited via a **different** arc (the two inner↔center petals are distinct arcs).
  - `Both` — node uniqueness and edge uniqueness.
- **Empty step:** walk along one circle-arc to the next node if it is empty. Cost 1.
- **Hop (default, `color_restricted_hops` on):** if the next node along a chosen circle and direction is occupied, consider the maximal consecutive occupied run along that same circle in that direction. You may spend **1 step** to skip the whole run and land on the first empty node beyond it, iff every hopped tile is the **mover’s color** and its weight compares to the mover with `hop_weight_cmp` (default `le`: occupant `<=` mover). Owner does not matter. Other colors cannot be hopped. Hopped nodes are marked visited; hopped arcs are marked used. With `color_restricted_hops` off, hopped tiles may be any color and `hop_requires_ge_weight` (default true) requires `occupant.weight >= mover.weight`.

Hops are circle-straight: they do not branch with the graph. Multi-arcs between the center and an inner node are distinct as circle-arcs (rendering the inner petals) but lead to the same neighbor for an empty step.

### Win and draw

- **Win (elimination):** the opponent has **no tiles on the board and an empty bench**. A remaining bag does not save them. If both players are eliminated in the same resolution, the game is a **draw**.
- **Win (locked-out):** if `no_moves_loses` is on (default) and the player to move has no legal ADD and no legal MOVE, they **lose immediately**. Detected at the start of their turn (after the previous player’s turn switch) and if they would otherwise pass.
- **Win (center hold):** if `center_hold_win` is on (default) and a player occupies the center at the end of `center_hold_n` consecutive **own** turns (default 5), they win.
- **Win (petal majority):** if `petal_majority_win` is on (default **off**) and a player occupies a majority of the inner-ring nodes (the 8 unique neighbors of the center — one attachment per petal) at the end of `petal_majority_n` consecutive **own** turns (default 1), they win.
- **Draw:** `turn_limit` completed player-turns (default **200**). A **double pass** draw exists only when `no_moves_loses` is off.

`turn_count` increments when a player’s action allotment ends (or they pass / have nothing left).

### Stall-prevention options

Each option is independently toggleable on `RuleSet` and via CLI. Greedy / minimax / MCTS share `legal_actions` / `apply`, so they honor every option. `LockedOut`, `CenterHold`, `PetalMajority`, `Control`, and `InsufficientTiles` terminals are scored as wins/losses (`±WIN`) in search from the **mover's** perspective (hold-style wins leave the winner to move; lockouts switch to the loser).

#### 1. `no_moves_loses` (default **on**)

CLI: `--no-moves-loses <on|off>`

If the player to move has no legal action — no ADD from their bench onto an empty spawn, and no legal MOVE — they **lose immediately** (`EndReason::LockedOut`). The winner is the opponent.

- PASS is not a legal action while this is on. `legal_actions` returns an empty list; applying PASS still records the lockout so bots that fall back to PASS end the game cleanly.
- The legacy **double-pass draw is disabled** while this is on (a pass never occurs). Turn-limit draws remain as a safety net.
- Running out of extra actions mid-turn after already taking a legal action (`actions_per_turn > 1`) still just ends the turn; lockout applies only when it is that player’s turn to act and they have nothing.

#### 2. `center_hold_win` (default **on**, N = 5)

CLI: `--center-hold-win <on|off>` `--center-hold-n <1..64>`

Each player has a counter of consecutive **own** turns that ended with their tile on the center node.

- At the end of player P’s turn: if P occupies the center, P’s counter increments; if it reaches `center_hold_n`, P wins (`EndReason::CenterHold`). If P does not occupy the center, P’s counter resets to 0.
- If the center is vacated or taken by the opponent, the previous holder’s counter resets (the opponent’s counter is unchanged until the end of *their* turn, when it will increment if they now hold it).
- Win is awarded at **exactly N** (the turn on which the counter becomes N).
- The HUD shows `center hold P1 a/N P2 b/N` while this option is on.

#### 3. `no_same_piece_twice` (default **WhenBenchEmpty**)

CLI: `--no-same-piece-twice <off|always|when-bench-empty>`

A player may not MOVE the same tile (by tile id) on two consecutive **own** turns.

- `Off` — no restriction.
- `Always` — always enforced.
- `WhenBenchEmpty` — enforced only while that player’s bench is empty (opening ADD-heavy play is unrestricted; endgame shuffling is not).

**Capture exception:** a repeat MOVE **is** legal if that MOVE causes at least one opponent tile to be removed under simultaneous overwhelm (previewed before the action). Own-tile suicides do not count. ADD does not set the “last moved” tile; only a MOVE does.

#### 4. `color_restricted_hops` (default **on**)

CLI: `--color-restricted-hops <on|off>` `--hop-weight-cmp <lt|le>`

**Interpretation** of the designer note *“If the weight in your color is less than, you can traverse.”* When this option is **on**, a hop may only cross a run of tiles that are:

1. the **mover’s color** (either owner), and
2. whose weight compares to the mover’s weight with `hop_weight_cmp`.

Default comparison is **`Le`** (occupant weight **less than or equal to** the mover). `Lt` is the stricter option (occupant **strictly less than** the mover). Owner still does not matter. Other colors cannot be hopped at all.

When this option is **off**, the legacy rule applies: hopped tiles may be any color/owner, and `hop_requires_ge_weight` (default true) requires `occupant.weight >= mover.weight`. The color-restricted regime **overrides** `hop_requires_ge_weight`.

This is the opposite inequality of the legacy hop (`color_restricted_hops` off: hop equal-or-heavier of any color; on: hop equal-or-lighter of your color only, or strictly lighter with `lt`).

#### 5. `petal_majority_win` (experimental, default **off**, N = 1)

CLI: `--petal-majority-win <on|off>` `--petal-majority-n <1..64>`

A **petal** here is one of the eight generating circles of the rosette. Each petal attaches to the center at exactly one unique inner-ring node (`Board::inner_ring()`, the 8 neighbors of the origin). Holding a **majority of petals** means occupying **more than half** of those nodes (5 of 8). Outer tips and spawn nodes are not counted.

- At the end of player P’s turn: if P occupies at least `need = inner_ring.len()/2 + 1` inner-ring nodes, P’s counter increments; if it reaches `petal_majority_n`, P wins (`EndReason::PetalMajority`). Otherwise P’s counter resets to 0.
- Default N = 1 (win at the end of the turn you first have 5 inner-ring tiles). N = 3 is the longer-hold setting used in benches.
- Only one player can have a majority at a time (5+5 > 8). If center-hold and petal-majority would fire on the same turn, **center-hold is checked first**.
- The HUD shows petal counts and hold progress while this option is on.

#### 6. `control_win` (experimental, default **off**, k = 5, t = 1)

CLI: `--control-win <on|off>` `--control-n <1..9>` `--control-hold-turns <1..64>`

The **control set** is the center (node 0) plus the 8 inner-ring nodes (`Board::inner_ring()`), 9 nodes total. A player wins by occupying at least `control_n` of these 9 (default 5) for `control_hold_turns` consecutive **own** turns (default 1 = end of the turn you first have 5).

- Grabbing the center alone is 1 of 9 and is **not** a win. This is meant as a center-hold replacement so a center rush is not a race to victory.
- At the end of player P’s turn: if P occupies at least `control_n` control-set nodes, P’s counter increments; if it reaches `control_hold_turns`, P wins (`EndReason::Control`). Otherwise P’s counter resets to 0. The opponent’s counter resets when they drop below `control_n`.
- Check order at end of turn: center-hold, then petal-majority, then control, then insufficient-tiles, then turn-limit, then lockout of the incoming player.
- Defaults stay off until a package is chosen from benches. Benches run with center-hold **off** and control-win **on**.
- Total game clocks (a draw condition) are a planned follow-up; they are not implemented here.

#### 7. `insufficient_tiles_loss` (experimental, default **off**) — "can't reach control"

CLI: `--insufficient-tiles-loss <off|strict|no-recapture>`

A player **loses** (`EndReason::InsufficientTiles`) when they can no longer occupy 5 control-set nodes: `board tiles + bench < control_n` (default 5). Bag tiles are not in play until drawn, and bag draws happen **only** on a MOVE that captures an opponent tile.

Checked at the **turn boundary** (end of `finish_turn`) for **both** players, after hold wins. If both are insufficient at once, the game is a **draw** (`EndReason::InsufficientTiles`).

- **`strict`**: lose as soon as `board + bench < 5`, ignoring the bag and ignoring whether a capture is available.
- **`no-recapture`**: lose when `board + bench < 5` **and** they cannot gain a tile. "Cannot gain" means the bag is empty, `capture_draw_on_move` is off, **or they have no path-legal MOVE that would remove an opponent tile in the current position** (previewed simultaneous overwhelm; same check as the capture exception, via `has_capturing_move`). The check does not require that player to be to-move, so both sides can be evaluated at the same turn boundary. A capturing MOVE with a non-empty bag saves them even if `board + bench < 5`. This is stricter than bag-empty-only: tiles left in the bag do not help if no capturing MOVE exists right now. Future captures on a later turn are not searched.

#### 8. `no_repeated_edge` / `path_constraint` (default **NoEdgeRevisit**)

CLI: `--path-constraint <no-node-revisit|no-edge-revisit|both>` and `--no-repeated-edge <on|off>` (alias: `off` → `NoNodeRevisit`; `on` keeps `NoEdgeRevisit` or upgrades `NoNodeRevisit` to `Both`).

An edge is an **undirected generating-circle arc** (the CW and CCW traversals of the same consecutive pair on one circle are the same edge). The two inner↔center petals on different circles are different edges.

Landing still requires an empty node other than the start.

**Empty-board unique destination counts from the center** (one tile, no other occupancy). Unique destinations are identical under all three constraints — extra return-through-the-second-petal paths exist but land on nodes already reachable by simple walks:

| dots | NoNodeRevisit | NoEdgeRevisit (default) | Both |
|---|---|---|---|
| 1 | 8 (inner ring) | 8 | 8 |
| 2 | 8 (spawns) | 8 | 8 |
| 3 | 16 (inners + outers) | 16 | 16 |
| 4 | 16 | 16 | 16 |

From an inner-ring node at d=3 the unique counts are also identical (14). Unique destinations **do** diverge in occupied positions: a 3-step inner → center → same inner (other petal) → spawn neighbor is legal under `NoEdgeRevisit` and illegal under `NoNodeRevisit` / `Both` when every other node is filled.

### Play

CLI (unknown args error out with usage):

```
apa-kan --bot1 <human|random|greedy|minimax|mcts> --bot2 <...>
        --seed N --think-ms N --delay-ms N --actions N --turn-limit N
        --screenshot PATH
        --no-moves-loses on|off
        --center-hold-win on|off --center-hold-n N
        --no-same-piece-twice off|always|when-bench-empty
        --color-restricted-hops on|off --hop-weight-cmp lt|le
        --petal-majority-win on|off --petal-majority-n N
        --control-win on|off --control-n K --control-hold-turns T
        --insufficient-tiles-loss off|strict|no-recapture
        --path-constraint no-node-revisit|no-edge-revisit|both
```

Defaults: P1 human, P2 greedy, seed 1, think 80 ms, `no_moves_loses` on, `center_hold_win` on (N=5), `no_same_piece_twice` when-bench-empty, `color_restricted_hops` on, `hop_weight_cmp` le, `petal_majority_win` off, `control_win` off, `insufficient_tiles_loss` off, `path_constraint` no-edge-revisit.

Bots: uniform random; greedy one-ply; minimax with alpha-beta, iterative deepening, and a material / at-risk / mobility / center-hold-threat heuristic (`minimax-legacy` is the previous eval); MCTS (UCT). Legal-move generation is shared, so every bot respects every RuleSet option. Locked-out, center-hold, petal-majority, control, and insufficient-tiles terminals are wins/losses in search.

Headless round-robin: `apa-kan-bench` (same rule flags; `--bots greedy,minimax,mcts`).

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

- Color-restricted hop weight: default is `le` (occupant `<=` mover). `lt` remains available as the stricter reading of “if the weight in your color is less than, you can traverse.”
- Whether `WhenBenchEmpty` should look at bench emptiness at the start of the turn or at MOVE time (implemented: at MOVE time).
