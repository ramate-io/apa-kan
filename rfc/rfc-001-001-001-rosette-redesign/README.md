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
- **Rings** (1–3 distinct colors). A **normal** ring indicates a color the tile can match (in and out) on the **normal channel**. A **zag** ring never matches on its own channel. A zag on either tile whose color the other tile holds (normal or zag) is a mismatch on the **zag channel** (`zag_mismatch`, default `sum`; `max` takes the per-color max of hitting zag strokes). Generation still draws distinct colors as in the six-color tree, then each ring independently becomes a zag with chance 1/7 (`zag_numer / 7`; `--zag-chance 0` / `--zags off` disables). You cannot have both a normal and a zag of the same color on a generated tile (one ring per color; stacked same-color zags exist only as test fixtures so SUM can add strokes and MAX can take a per-color max). A **braid** occupies 2 or 3 ring slots (1–3 colors; single-color braids allowed) and is satisfied when the neighbor’s **ring** color counts cover it (braid-channel match = braid size). An orange+orange braid needs two orange rings on the neighbor. Unsatisfied braids score **one** +1 mismatch for the edge (`--braid-fail-mismatch one`, the accepted default; `weight` / `sum` are research variants, [issue #39](https://github.com/ramate-io/apa-kan/issues/39)) and draw each side's own braid weave into a midpoint diamond. One-ended braids draw the would-be weave. Per edge, take the largest fulfilled braid (no summing). Channels are independent: a zag and a braid on the same edge both score, and a leftover normal match stacks with a fulfilled braid. `--braid-chance` / `--braids` default 1/7. Bead / antipodal ring types are not implemented. Colors: green, blue, orange (weight 81 each), lavender-pink, teal (9 each), maroon (1). Ring 1 always exists. Ring 2: weight 7 same as ring 1, weight 1 any color from the same tree (a redraw of ring 1 is not a new ring). Ring 3: same rule relative to the last existing ring. `--max-rings` caps this at 1–3 (default 3).
- **Weight** is the live ring-slot count (`n_rings`, braid slots included). Overwhelm match base and hop compare use that count. There is no separate hidden weight, and painters do not draw extra concentric copies of the last ring.
- **Dots** `d ∈ {1,2,3}` drawn as 1–3 dots (MOVE length).

Dots are independent of ring colors. Weight is not: it is exactly the number of rings.

### Deck and spawn tokens (`RuleSet` options)

These are two separate concepts.

1. **Tiles in your deck.** Each player has a public **`bag_size`-tile deck** (default 11; CLI `--deck-size` / `--bag-size <1..27>`). Ring process × uniform dots 1–3, shuffled with a seed. Weight is the resulting ring-slot count. The old 27 unique 3-color kinds no longer fit the six-color / multi-ring space. `--deck-size 27` restores the historical full bag. **All 11 tiles start unplaced and selectable.** There is no hidden bag and no bench/bag split. On any turn you may select **any unplaced tile** from your deck to ADD.
2. **Spawn tokens.** You spend one to ADD a deck tile to the board. Start with **`starting_bench_size` tokens** (default 7; CLI `--starting-tokens` / `--starting-bench`). Independent of deck size. Earn one per capture (opponent capture, and self-capture/sacrifice when `self_capture_budget` is on).

Default:

- **`identical_bags` (default true):** both players get the same shuffle (new tile ids / owners), for fairness.
- **Usable budget** is `min(tokens in the stack, unplaced deck tiles)`. Tokens beyond the remaining deck are still earned and still shown (the stack uses opaque blank faces for surplus) but do not count as budget for ADD legality, lockout, low-value, or eval.
- **ADD** places one concrete remaining-deck tile onto an empty spawn. Any unplaced tile may be chosen at any time; the only ADD gate is **usable budget > 0** (plus an empty spawn). ADD spends 1 spawn token (popped from the front of the stack, so opening blanks spend first).
- **Spawn tokens:** each opponent capture on your turn earns +1 token. Capturing your **own** tile (self-capture already happens under simultaneous overwhelm; it is not a new capture rule) also earns +1 when `self_capture_budget` is on (default; CLI `--self-capture-budget on|off`). The captured tile is pushed onto the token stack only when it earns a token. Captures do **not** draw extra tiles. `capture_draw_on_move` remains in `RuleSet` (default **off**) so historical JSONL logs replay.
- Remaining hidden-bag tiles (legacy logs only) do **not** keep a player alive. `is_alive` is board tiles or unplaced deck tiles.
- **Scenarios** (`--scenario <name|path>`, TOML under `apa-kan/game/scenarios/`): replace the default deal with a named or file-backed deck. Builtins: `default`/`same` (`generative = true`, mirrored `bag_size`-tile sample, default 11); `independent` (each player sampled separately); `complementary` (P1 green+orange, P2 blue+orange, 18 one-ring tiles, mirrored dots order; weight is always 1); `complementary-swapped` (exclusive colors flipped). Optional per-player `colors` / `tiles` (`color` or `colors = [...]`) / `bench` (ordered prefix of the public deck). The app and `apa-kan-bench` share the loader. Generative scenarios honor `--deck-size` / `--bag-size`. TOML `weight` is accepted and ignored; tile weight is the ring-slot count.

Options: `starting_bench_size` (spawn tokens), `bag_size` (public deck), `self_capture_budget`, `identical_bags`, `bag_one_of_each_kind`, `capture_draw_on_move` (legacy), plus scenario files.

### Overwhelm (by color)

Evaluated **board-wide, simultaneously**, after every action.

**Independent channels** (see [`rfc-001-001-002-independent-channels`](../rfc-001-001-002-independent-channels/README.md)) replace the earlier cross-type overrides. Core: every ring has weight 1; `effective_weight = mismatch − match` summed over edges; overwhelmed iff `effective_weight >= ring_weight`. Per edge, normal / zag / braid each score on their own and **add**. A zag vs a same-color normal nets 0; a fulfilled braid stacks with a normal match. Failed braid **accepted default** is +1 (`--braid-fail-mismatch one`). `weight` and `sum` stay implemented for later research in [issue #39](https://github.com/ramate-io/apa-kan/issues/39). Doubles are **allowed** by default (`--no-same-piece-twice off`).

- A neighbor **matches** on the **normal** channel if some normal-ring color of either tile is in the other tile's color multiset (including zag and braid colors). Zags never produce a zag-channel match.
- Zag mismatches are an **edge** event (`RuleSet::zag_mismatch`, CLI `--zag-mismatch`): a zag on either tile whose color the other holds (normal or zag) counts. Both tiles take the same number. The zag channel no longer suppresses the normal lane.
  - **`sum` (default):** count every hitting zag stroke on both tiles. A orange zag vs B orange zag = 2. 3 blue zags vs 3 blue zags = 6. 2 orange vs 1 orange = 3.
  - **`max`:** for each color, the edge count for that color is the **max**, over the two tiles, of how many of that tile's zag strokes of that color hit the other (the other holds that color on any ring). The edge total is the sum of those per-color maxima. Orange zag vs orange zag = 1. 3 blue vs 3 blue = 3. 2 orange vs 1 orange = 2. A 2 orange zags (B holds orange) + B 1 green zag (A holds green) = 3.
- Example: blue normal + orange zag vs the same → 1 blue **normal** match and **2** zag mismatches under `sum` (both orange zags hit), or **1** under `max`. Two orange zags on U vs T carrying orange → 2 under both `sum` and `max`.
- Blue-normal vs orange-normal + green-zag is a mismatch on the **normal** channel (blue vs orange); the green zag does not hit blue. Green-zag vs orange-normal + two green normals is a one-sided zag hit (both tiles take 1) plus a **normal** match (green). Blue-normal + orange-zag vs green-normal + orange-normal is a **normal** match (orange) **and** a zag hit of 1 — they add; the zag no longer replaces the ordinary hitch.
- `matches = own ring count + (sum of match-weight over incident generating-circle arcs)`
- Inner↔center is **two** such arcs (two petals). Unique neighbor-sets are for occupancy (petal majority); overwhelm walks arcs, same as MOVE.
- Removed iff `mismatches >= matches`
- A one-ring zag is inert in isolation. On contact with a tile that carries that color it never matches, so a 1-ring zag is captured under the ordinary rule; a 3-ring color-carrier takes the zag hit and may survive.

The designer example: a **double green** next to two oranges has matches 2 / mismatches 2 and is overwhelmed; a green neighbor makes matches 3, so it survives. Adjacent same-color tiles of the opponent still count as matches.

**`mismatch_uses_neighbor_weight` (default false):** if on, each mismatch adds the neighbor’s ring count instead of 1.

### Actions

One action per turn by default (`actions_per_turn`, default 1):

1. **ADD** any remaining bench tile to an empty spawn, if you have usable capture budget. Legal even if the tile would be immediately overwhelmed; the UI warns. Spends 1 token.
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
- **Empty step:** walk along any unused adjacency (a generating-circle arc) to a neighboring empty node. Cost 1. Inner↔center still has two distinct arcs.
- **Counted pass-through (default, `counted_pass_through` on):** hops are disabled. Each step is still to an adjacent node and costs 1. An occupied tile may be **passed through** (never landed on) iff it **shares a normal ring color** with the mover and its ring count compares with `pass_through_ring_cmp`. Default **`ge`**: occupant `n_rings >=` mover. **`gt`** is strict (`>`), so a 3-ring tile can never pass through (max rings is 3). Zag colors do not enable pass-through. Passing through counts as one of the exact-`d` steps (a two-tile occupied run plus a landing needs `d=3`). `NoEdgeRevisit` still applies. Capture/overwhelm resolves after landing as usual. CLI: `--counted-pass-through on|off`, `--graph-hops on|off` (alias: hops on ⇔ pass-through off), `--pass-through-ring-cmp ge|gt`.
- **Hop (optional, `--graph-hops on` / `counted_pass_through` off, `color_restricted_hops` on):** if a neighboring node is occupied, flood any adjacency through the connected chain of **hoppable** occupied tiles — each must **share a normal ring color** with the mover and compare with `hop_weight_cmp` (default `le`: occupant ring count `<=` mover). The path may change direction at any node, **including the center**. Spend **1 step** to land on any empty node adjacent to that chain. Owner does not matter. Zag colors do not enable hops. Hopped nodes are marked visited; hopped arcs are marked used (`NoEdgeRevisit` still applies). You cannot land on an occupied node. With `color_restricted_hops` off, hopped tiles may be any color and `hop_requires_ge_weight` (default true) requires occupant rings `>=` mover rings.

A hop over a chain is still one step of the exact-`d` MOVE. Empty walks, pass-throughs, and hops all follow unique neighbor adjacency; the two inner↔center petal arcs remain distinct unused-edge choices.

### Edge rendering (realized state)

Two-ended edges (both nodes occupied) draw **one lane per firing channel**, in order **normal, zag, braid**. Quiet channels are omitted. See [`rfc-001-001-002`](../rfc-001-001-002-independent-channels/README.md).

- **Normal match** → highway stripe in the matched color(s). **Normal mismatch** → diamond of unmatched leftover **normal** colors (braid threads are not mixed in).
- **Zag** → each triggered zag between the tiles, bundled on the zag lane (`sum` / `max` stroke counts unchanged). A non-hitting zag is not drawn on that edge.
- **Braid** → the max fulfilled weave, or a failed per-side weave that meets at a midpoint diamond (each tile uses only its own braid colors; a one-sided fail reaches the diamond from that side only). Unconnected edges still show the would-be braid.
- Nothing firing → grey base arc.

Tile-rim braid weave period is unchanged (`TILE_BRAID_TURN_UNITS = 0.21`). Tile-rim braid threads and zag rings are 1.3× thicker than the previous 0.70 / 0.90 strokes; arc strokes are unchanged. Mobile crowding of stacked lanes is acceptable for now.

One-ended edges (the tile has no neighbor on that arc) still draw all of that tile's **potential** effects: a would-be braid as the threaded weave in the braid's ring colors, leftover normal colors as a highway, and every zag, so it is clear what would be in play if another tile landed there.

### Win and draw

- **Win (elimination):** the opponent has **no tiles on the board and an empty remaining deck**. A leftover hidden bag (legacy logs) does not save them. If both players are eliminated in the same resolution, the game is a **draw**.
- **Win (locked-out):** if `no_moves_loses` is on (default) and the player to move has no legal ADD and no legal MOVE, they **lose immediately**. Detected at the start of their turn (after the previous player’s turn switch) and if they would otherwise pass.
- **Win (low-value):** if **both** players have `board tiles + usable budget` strictly below 5 (`LOW_VALUE_BOARD_LT`), the game ends. Usable budget is `min(tokens, unplaced deck tiles)` and **need not be 0**: a side with 1 on the board and 3 unplaced tiles (usable 3) is already below 5. Surplus tokens on an empty remaining deck do not count toward possible pieces. Sort each player's **on-board** pieces ascending by value and compare pair `i` (P1's `i`-th lowest vs P2's `i`-th lowest), starting at `i = 0`. Value weights live in `PieceValueWeights` (easy to extend when new ring types appear):
  1. **Primary:** `dots * 1 + normal_rings * 1 + zag_rings * 1` (every ring counts 1). Lower wins.
  2. **Tie-break:** recompute with `zag_rings * 2` (`PieceValueWeights::TIEBREAK`). Lower wins. A 1-dot/1-zag loses to a 1-dot/1-normal.
  3. Both passes tied: advance to pair `i + 1`.
  4. A **missing** piece is treated as maximum value, so the player with more surviving tiles wins that pair.
  5. **Draw** only if both players have the same piece count and every pair ties on both passes.
  Checked **after lockout** of the incoming player: someone with no legal action still loses by lockout even if they hold the lowest piece. Control / hold wins and the turn-limit are still earlier in `finish_turn`.
- **Win (center hold):** if `center_hold_win` is on (default **off**) and a player occupies the center at the end of `center_hold_n` consecutive **own** turns (default 5), they win.
- **Win (control):** if `control_win` is on (default) and a player occupies at least `control_n` of the 9-node control set (center + inner ring; default 5 of 9) at the end of `control_hold_turns` consecutive **own** turns (default 1), they win.
- **Win (petal majority):** if `petal_majority_win` is on (default **off**) and a player occupies a majority of the inner-ring nodes (the 8 unique neighbors of the center — one attachment per petal) at the end of `petal_majority_n` consecutive **own** turns (default 1), they win.
- **Draw:** `turn_limit` completed player-turns (default **200**). A **double pass** draw exists only when `no_moves_loses` is off.

`turn_count` increments when a player’s action allotment ends (or they pass / have nothing left).

### Stall-prevention options

Each option is independently toggleable on `RuleSet` and via CLI. Greedy / minimax / MCTS share `legal_actions` / `apply`, so they honor every option. `LockedOut`, `CenterHold`, `PetalMajority`, `Control`, `InsufficientTiles`, and `LowValue` terminals are scored as wins/losses (`±WIN`) in search from the **mover's** perspective (hold-style wins leave the winner to move; lockouts switch to the loser; low-value is checked after lockout). A low-value draw (equal counts, every pair tied) scores 0.

#### 1. `no_moves_loses` (default **on**)

CLI: `--no-moves-loses <on|off>`

If the player to move has no legal action — no ADD (zero usable budget or no empty spawn), and no legal MOVE — they **lose immediately** (`EndReason::LockedOut`). The winner is the opponent. Empty bench is zero usable budget even when leftover tokens remain.

- PASS is not a legal action while this is on. `legal_actions` returns an empty list; applying PASS still records the lockout so bots that fall back to PASS end the game cleanly.
- The legacy **double-pass draw is disabled** while this is on (a pass never occurs). Turn-limit draws remain as a safety net.
- Running out of extra actions mid-turn after already taking a legal action (`actions_per_turn > 1`) still just ends the turn; lockout applies only when it is that player’s turn to act and they have nothing.

#### 2. `center_hold_win` (default **off**, N = 5)

CLI: `--center-hold-win <on|off>` `--center-hold-n <1..64>`

Each player has a counter of consecutive **own** turns that ended with their tile on the center node.

- At the end of player P’s turn: if P occupies the center, P’s counter increments; if it reaches `center_hold_n`, P wins (`EndReason::CenterHold`). If P does not occupy the center, P’s counter resets to 0.
- If the center is vacated or taken by the opponent, the previous holder’s counter resets (the opponent’s counter is unchanged until the end of *their* turn, when it will increment if they now hold it).
- Win is awarded at **exactly N** (the turn on which the counter becomes N).
- The HUD shows `center hold P1 a/N P2 b/N` while this option is on.

#### 3. `no_same_piece_twice` (default **Off**)

CLI: `--no-same-piece-twice <off|always|when-deck-empty>` (`when-bench-empty` is still accepted)

A player may not MOVE the same tile (by tile id) on two consecutive **own** turns.

- `Off` — no restriction (**accepted default**; doubles allowed).
- `Always` — always enforced (non-default variant).
- `WhenDeckEmpty` — enforced only while that player’s remaining (unplaced) deck is empty (opening ADD-heavy play is unrestricted; endgame shuffling is not). Non-default variant.

**Capture exception:** a repeat MOVE **is** legal if that MOVE causes at least one opponent tile to be removed under simultaneous overwhelm (previewed before the action). Own-tile suicides do not count. ADD does not set the “last moved” tile; only a MOVE does.

#### 4. `color_restricted_hops` (default **on**; hops themselves are off unless `--graph-hops`)

CLI: `--color-restricted-hops <on|off>` `--hop-weight-cmp <lt|le>` `--graph-hops <on|off>`

**Interpretation** of the designer note *“If the weight in your color is less than, you can traverse.”* When this option is **on**, a hop may only cross a connected chain of tiles that are:

1. sharing a **normal** ring color with the mover (either owner), and
2. whose ring count compares to the mover’s ring count with `hop_weight_cmp`.

Default comparison is **`Le`** (occupant rings **less than or equal to** the mover). `Lt` is the stricter option (occupant **strictly less than** the mover). Owner still does not matter. Other colors cannot be hopped at all.

When this option is **off**, the legacy rule applies: hopped tiles may be any color/owner, and `hop_requires_ge_weight` (default true) requires occupant rings `>=` mover rings. The color-restricted regime **overrides** `hop_requires_ge_weight`.

This is the opposite inequality of the legacy hop (`color_restricted_hops` off: hop equal-or-heavier of any color; on: hop equal-or-lighter of your color only, or strictly lighter with `lt`).

#### 5. `petal_majority_win` (experimental, default **off**, N = 1)

CLI: `--petal-majority-win <on|off>` `--petal-majority-n <1..64>`

A **petal** here is one of the eight generating circles of the rosette. Each petal attaches to the center at exactly one unique inner-ring node (`Board::inner_ring()`, the 8 neighbors of the origin). Holding a **majority of petals** means occupying **more than half** of those nodes (5 of 8). Outer tips and spawn nodes are not counted.

- At the end of player P’s turn: if P occupies at least `need = inner_ring.len()/2 + 1` inner-ring nodes, P’s counter increments; if it reaches `petal_majority_n`, P wins (`EndReason::PetalMajority`). Otherwise P’s counter resets to 0.
- Default N = 1 (win at the end of the turn you first have 5 inner-ring tiles). N = 3 is the longer-hold setting used in benches.
- Only one player can have a majority at a time (5+5 > 8). If center-hold and petal-majority would fire on the same turn, **center-hold is checked first**.
- The HUD shows petal counts and hold progress while this option is on.

#### 6. `control_win` (default **on**, k = 5, t = 1)

CLI: `--control-win <on|off>` `--control-n <1..9>` `--control-hold-turns <1..64>`

The **control set** is the center (node 0) plus the 8 inner-ring nodes (`Board::inner_ring()`), 9 nodes total. A player wins by occupying at least `control_n` of these 9 (default 5) for `control_hold_turns` consecutive **own** turns (default 1 = end of the turn you first have 5).

- Grabbing the center alone is 1 of 9 and is **not** a win. This is meant as a center-hold replacement so a center rush is not a race to victory.
- At the end of player P’s turn: if P occupies at least `control_n` control-set nodes, P’s counter increments; if it reaches `control_hold_turns`, P wins (`EndReason::Control`). Otherwise P’s counter resets to 0. The opponent’s counter resets when they drop below `control_n`.
- Check order at end of turn: center-hold, then petal-majority, then control, then insufficient-tiles, then turn-limit, then lockout of the incoming player, then low-value (both players' `board + usable budget` strictly below 5).
- The HUD shows `ctl P1 a/k h/t P2 b/k i/t` while this option is on.
- Chosen default package (from [`control_win_t1_bench7.txt`](../../apa-kan/game/benchmark_results/control_win_t1_bench7.txt)): control-win **on** (5 of 9, t=1), center-hold **off**, starting bench 7, insufficient-tiles **off**, petal-majority **off**. Restore the previous package with `--center-hold-win on --control-win off --starting-bench 5`.
- Total game clocks (a draw condition) are a planned follow-up; they are not implemented here.

#### 7. `insufficient_tiles_loss` (experimental, default **off**) — "can't reach control"

CLI: `--insufficient-tiles-loss <off|strict|no-recapture>`

A player **loses** (`EndReason::InsufficientTiles`) when they can no longer occupy 5 control-set nodes: `board tiles + remaining deck < control_n` (default 5). Captures earn spawn tokens, not extra deck tiles.

Checked at the **turn boundary** (end of `finish_turn`) for **both** players, after hold wins. If both are insufficient at once, the game is a **draw** (`EndReason::InsufficientTiles`).

- **`strict`**: lose as soon as `board + remaining deck < 5`.
- **`no-recapture`**: lose when `board + remaining deck < 5` **and** they cannot gain a tile. With the public deck there is no hidden remainder, so this matches strict unless legacy `capture_draw_on_move` is on and they have a path-legal MOVE that would remove an opponent tile (previewed simultaneous overwhelm; same check as the capture exception, via `has_capturing_move`). The check does not require that player to be to-move, so both sides can be evaluated at the same turn boundary. Future captures on a later turn are not searched.

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
        --no-same-piece-twice off|always|when-deck-empty
        --color-restricted-hops on|off --hop-weight-cmp lt|le
        --counted-pass-through on|off --graph-hops on|off
        --pass-through-ring-cmp ge|gt
        --petal-majority-win on|off --petal-majority-n N
        --control-win on|off --control-n K --control-hold-turns T
        --insufficient-tiles-loss off|strict|no-recapture
        --starting-tokens N --deck-size N --self-capture-budget on|off
        --max-rings 1..3 --zag-chance 0..7 --zags on|off
	   --zag-mismatch sum|max --braid-fail-mismatch one|weight|sum
        --scenario name|path --list-scenarios
        --path-constraint no-node-revisit|no-edge-revisit|both
```

Defaults: P1 human, P2 greedy, seed 1, think 80 ms, `no_moves_loses` on, `center_hold_win` off, `no_same_piece_twice` off (doubles allowed), `color_restricted_hops` on, `hop_weight_cmp` le, `counted_pass_through` on, `pass_through_ring_cmp` ge, `petal_majority_win` off, `control_win` on (k=5, t=1), `insufficient_tiles_loss` off, `starting_bench_size` 7 (spawn tokens), `bag_size` 11 (public deck), `self_capture_budget` on, `max_rings` 3, `zag_numer` 1 (1/7), `zag_mismatch` sum, `--braid-fail-mismatch one`, `path_constraint` no-edge-revisit. Usable budget is `min(tokens, unplaced deck tiles)`. Low-value fires when both players have `board + usable budget < 5`, then uses the progressive lowest-tile pair check (missing piece = max value). The accepted default ruleset (independent channels, flat-1 failed braids, public 11-tile deck with spawn tokens, doubles allowed) is documented in [`rfc-001-001-002-independent-channels`](../rfc-001-001-002-independent-channels/README.md).

Bots: uniform random; greedy one-ply; minimax with alpha-beta, iterative deepening, and a material / at-risk / mobility / usable-budget / center-hold-threat heuristic (`minimax-legacy` is the previous eval); MCTS (UCT). Legal-move generation is shared, so every bot respects every RuleSet option. Eval and search read usable budget (`min(tokens, remaining deck)`), not leftover surplus, and see every unplaced tile (the deck is public). Locked-out, center-hold, petal-majority, control, insufficient-tiles, and low-value terminals are wins/losses in search (a low-value equal-count full tie is a draw scored 0), including InProgress positions where both `board + usable` counts are already below 5. Search sees the same progressive pair check as the rules, including leftover tiles beating an empty slot.

Headless round-robin: `apa-kan-bench` (same rule flags and `--scenario`; `--bots greedy,minimax,mcts`). Logs include per-game opening benches and a Pearson correlation of a simple deck-strength measure (sum of weights, dots, color count) with outcome.

## Implementation

| Module | Role |
|---|---|
| `board` | Geometric graph |
| `game` | RuleSet, tiles, actions, overwhelm, win |
| `scenario` | Named/TOML opening decks |
| `ai` | Bots |
| `ui` | Bevy 0.19.1 table |
| `render` | Headless PNG |
| `config` | Fail-closed CLI |
| `src/bin/bench.rs` | Round-robin |

## Alternatives considered

- Unique-neighbor degree 4 for every non-center node — false for the inner ring; arc-degree 4 is the geometric fact.
- Infinite 27-kind picker with an ADD counter — rejected; the deck is still concrete sampled tiles. Spawn tokens are an ADD counter **on top of** that concrete hand: you pick any remaining deck tile, and usable budget (min(tokens, unplaced tiles)) is the only ADD gate. Surplus tokens are kept until another unplaced tile remains rather than discarded.
- MOVE “up to d” — rejected; exact `d`.
- Overwhelm by owner — rejected; color, as in the original game.

## Open questions

- Color-restricted hop weight: default is `le` (occupant `<=` mover). `lt` remains available as the stricter reading of “if the weight in your color is less than, you can traverse.”
- Whether `WhenDeckEmpty` should look at remaining-deck emptiness at the start of the turn or at MOVE time (implemented: at MOVE time).
