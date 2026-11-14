# RFC 001-001-002: Independent additive edge channels

## Problem

Edge scoring in `rfc-001-001-001` used **cross-type overrides**:

- a zag hit replaced the ordinary +1 mismatch (no diamond beside a zag)
- a fulfilled braid suppressed a leftover normal match
- a failed braid hitch-mismatched only when nothing else scored

Those overrides made a same-color zag vs a normal a capture instead of a wash, and they hid a satisfied braid's leftover normal match. This RFC replaces that with **independent, additive per-type channels**.

## Accepted default ruleset

Together with [`rfc-001-001-001-rosette-redesign`](../rfc-001-001-001-rosette-redesign/README.md), the accepted default is:

- **Independent additive channels** (this RFC): per edge, normal / zag / braid score on their own and add. No cross-channel overrides.
- **Failed-braid mismatch `--braid-fail-mismatch one`** (flat +1). `weight` and `sum` stay implemented for later research in [issue #39](https://github.com/ramate-io/apa-kan/issues/39).
- **Public 11-tile deck with spawn tokens:** `bag_size` 11, `starting_bench_size` 7. ADD is gated by usable budget = min(tokens, unplaced deck tiles).
- **Doubles allowed:** `--no-same-piece-twice off` is the locked default and the **only benchmarked** H2H configuration. `always` and `when-deck-empty` remain as non-default CLI variants; they are not part of the H2H package.
- **Low-value:** both players have `board tiles + usable budget` strictly below 5. Usable budget need not be 0. Progressive pair resolution is unchanged (missing on-board piece = max value).

Supported by the committed [`work_budget_doubles_allowed`](../../apa-kan/game/benchmark_results/work_budget_doubles_allowed) H2H (12 games/pairing/seat, seed 1, `--no-depth-probe`, 108 games, `--budget nodes:132000,iters:2600`, `--jobs 4`) under per-arc inner↔center scoring and this low-value trigger: P1 **45-17-46**, mean avg turns **60.29**. The earlier time-based [`center_double_edge_doubles_allowed`](../../apa-kan/game/benchmark_results/center_double_edge_doubles_allowed) log (P1 **53-8-47**, mean **47.47**) is historical. Doubles-disallowed logs ([`work_budget_doubles_disallowed`](../../apa-kan/game/benchmark_results/work_budget_doubles_disallowed), [`center_double_edge_doubles_disallowed`](../../apa-kan/game/benchmark_results/center_double_edge_doubles_disallowed), [`doubles_disallowed.txt`](../../apa-kan/game/benchmark_results/doubles_disallowed.txt)) are **historical** and must not be overwritten or rerun as the default package. The unique-neighbor [`doubles_allowed`](../../apa-kan/game/benchmark_results/doubles_allowed.txt) / [`independent_channels`](../../apa-kan/game/benchmark_results/independent_channels.txt) files predate both the inner↔center double-edge scoring fix and this trigger and are kept for comparison.

## Proposed solution

### Core (unchanged)

These already hold; anything else is a bug.

- Every ring has weight 1. A tile's `ring_weight` equals its ring count (`n_rings`), including braid slots.
- Per edge, mismatch is positive and match is negative.
- `effective_weight` = Σ over the tile's edges of (`mismatch_weight − match_weight`).
- A tile is overwhelmed when `effective_weight >= ring_weight`.

HUD `tile_score` still reports `(ring_weight + match_weight, mismatch_weight)`. `mismatches >= matches` is the same inequality.

### Channels

A channel contributes on an edge only if **at least one** of the two tiles has a ring of that type. Channels never override each other. "Colors_X" means all ring colors on tile X, as a **multiset** including braid and zag colors.

| Channel | When it fires | Match | Mismatch |
|---|---|---|---|
| **Normal** | some ring on A or B is normal | 1, if some normal-ring color of A is in Colors_B **or** some normal-ring color of B is in Colors_A | 1 otherwise (`mismatch_uses_neighbor_weight` still uses the neighbor's ring count) |
| **Zag** | some ring on A or B is a zag | never | +1 for each zag color on one side that appears among the other side's colors, counted as a multiset union over both directions (`zag_mismatch` `sum` / `max`, same formula as today) |
| **Braid** | some tile has a 2- or 3-slot braid | the single largest fulfillable braid on either side (`−2` or `−3`). Tied sizes pick either | if **no** braid on the edge can be fulfilled: **exactly one** mismatch of weight **+1** (`one`, **accepted default**). `weight` / `sum` are research variants ([issue #39](https://github.com/ramate-io/apa-kan/issues/39)) |

A braid is fulfilled when the other tile's color multiset covers the braid's color multiset (orange+orange needs two orange rings).

**Intended consequences**

- A zag on A plus a normal ring of the same color on B nets 0.
- A satisfied braid **stacks** with a normal match.

**Accepted default:** `--braid-fail-mismatch one` (flat +1 for the edge). `weight` (largest braid's slots) and `sum` (every braid's slots on both sides; 2+3 = +5) stay implemented for later research in [issue #39](https://github.com/ramate-io/apa-kan/issues/39).

### Center adjacency (two petals, two scoring edges)

The origin has **8 unique neighbors** (inner ring) and **16 incident generating-circle arcs**. Each inner↔center pair shares **two petals** — two undirected arcs on different circles through the same two nodes. Unique `neighbors()` collapses those to one adjacency; **scoring, capture, and lane paint iterate incident arcs**, so each petal is a separate edge. Failed-braid `one` is +1 **per petal**. Both tiles take that same +1 on each petal (undirected; not +1 per side of a collapsed pair). Inner-ring nodes are not adjacent to each other.

Worked example under the accepted default: a 2-slot orange+green braid at the center against two inner 2-slot blue+teal braids (neither braid covers the other).

One petal occupied:

| Tile | `ring_weight` | occupied petals | signed | overwhelmed? |
|---|---|---|---|---|
| center | 2 | +1 + +1 | **+2** | **yes** (2 ≥ 2) |
| that petal | 2 | +1 + +1 | **+2** | **yes** (2 ≥ 2) |

Two petals occupied (the reported position):

| Tile | `ring_weight` | occupied petals | signed | overwhelmed? |
|---|---|---|---|---|
| center | 2 | +1 × 4 | **+4** | **yes** |
| each petal | 2 | +1 + +1 | **+2** | **yes** |

Capture is simultaneous: `collect_removals` scores every occupied node on the current board, then `apply` clears all of those nodes. Neighbors are evaluated from that pre-removal state. `game::tests::failed_double_braid_at_center_pops_center_and_both_petals` and `visual::tests::both_center_petal_arcs_paint_failed_braid_and_match` lock this.

### Plain English

- **Normal.** If either tile has a normal ring, the edge either likes the pair (they "see" a shared color from a normal into the neighbor's rings) or it doesn't. One point either way. Leftover extra colors do not add extra hitch points once a match exists.
- **Zag.** Zags only punish. Count every hitting zag stroke on the edge (SUM by default). They never make a match of their own; a same-color normal on the other tile is the normal channel's job.
- **Braid.** Look at both braids, keep the biggest one that the neighbor can cover, and score that hitch as a match of its size. If neither braid is covered, the edge takes a single +1 (`one`, the accepted default). Heavier `weight` / `sum` costs are tracked in [issue #39](https://github.com/ramate-io/apa-kan/issues/39).

## Worked table

Signs: match is written as a positive match-weight in `(match, mismatch)` — the signed edge is `mismatch − match`.

| Case | A | B | Normal | Zag | Braid | Edge `(m, x)` | signed | Notes |
|---|---|---|---|---|---|---|---|---|
| Zag vs normal same color | orange zag | orange normal | match 1 | +1 | — | (1, 1) | 0 | wash; neither 1-ring tile is overwhelmed |
| Braid plus normal stacking | orange+green braid, orange normal | orange+green normals | match 1 | — | match 2 | (3, 0) | −3 | leftover orange stacks |
| Both sides with braids, one fulfills | orange+green+blue braid | orange+green braid + blue normal | match 1 | — | match 3 (largest) | (4, 0) | −4 | do not sum 2+3 |
| Neither braid fulfilled | orange+green braid | blue+teal braid | — | — | mismatch 1 | (0, 1) | +1 | one fail for the edge, not per side |
| Zag-only next to a normal-only | orange zag | blue normal | mismatch 1 | 0 (no hit) | — | (0, 1) | +1 | zag does not cancel the hitch |
| Same-color braid needing two rings | orange+orange braid | one orange normal | match 1 | — | mismatch 1 | (1, 1) | 0 | one orange cannot cover orange+orange; the normal still matches |

Unit tests in `game::tests::independent_channel_table_rows` cover every row.

## Visual lane spec

Each channel that **fires** (nonzero ink: a normal match or hitch, at least one hitting/potential zag, or a braid weave/fail) gets its own lane. Fixed order: **normal, zag, braid**. Quiet channels are omitted.

Lane offsets, given pitch `≈ 2.4 ×` highway stripe offset:

| Active lanes | Offsets (normal, zag, braid) |
|---|---|
| 1 | centerline |
| 2 | `+pitch`, `−pitch` in the order they appear |
| 3 | `+pitch`, `0`, `−pitch` |

**Normal**

- Match: a highway stripe in the matched color(s) — one thick solid, or two parallel strands. Same glyph as today's match highway.
- Mismatch: a diamond stripe with all unmatched leftover **normal** colors (today's hitch, without braid threads mixed in).

**Zag**

- Draw each matched/triggered zag between the two tiles, bundled on the zag lane (`sum` / `max` stroke counts unchanged).

**Braid**

- Fulfilled: draw the max fulfilled braid between the two tiles (existing weave: one thread per slot, arc period `BRAID_TURN_UNITS = 0.42`).
- Failed: like the normal mismatch. From each tile, **that tile's own** braid weave reaches the middle (its threads only) and the two meet at a diamond. If only one side has a braid, only that side reaches in, up to the diamond.
- Unconnected edges still show the would-be braid (potential weave, no diamond).

Tile-rim weave period is unchanged (`TILE_BRAID_TURN_UNITS = 0.21`). Tile-rim braid threads and zag rings are 1.3× thicker than the previous 0.70 / 0.90 strokes; arc braid/zag strokes are unchanged. Mobile crowding of stacked lanes is acceptable for now.

## Implementation

- `EdgeChannels` / `Game::edge_channels` in `apa-kan/game/src/game.rs`
- Overwhelm, `tile_score`, and `would_overwhelm_add` walk `Board::incident_others` (generating-circle arcs). Unique `neighbors()` is occupancy only. Inner↔center is two scoring edges.
- `BraidFailMismatch` on `RuleSet` (`--braid-fail-mismatch one|weight|sum`)
- `ArcPaint.braid: Option<BraidLane>` plus lane offsets in `visual.rs`; renderers in `render.rs` / `ui.rs` already iterate `board.arcs()`, so both center-petal petals paint
- JSONL writes `braid_fail_mismatch` and `score_incident_arcs`. Historical logs omit those fields; full replay of those games is skipped because captures used old overrides or unique-neighbor scoring
- Current H2H (doubles allowed only): [`work_budget_doubles_allowed`](../../apa-kan/game/benchmark_results/work_budget_doubles_allowed) (P1 **45-17-46**). Historical time-based allowed: [`center_double_edge_doubles_allowed`](../../apa-kan/game/benchmark_results/center_double_edge_doubles_allowed) (P1 **53-8-47**). Doubles-disallowed logs are historical and not rerun; see [`benchmark_results/README.md`](../../apa-kan/game/benchmark_results/README.md)

## Alternatives considered

- Keep zag-hits replacing the ordinary mismatch — rejected; a same-color zag vs normal should net 0.
- Failed-braid mismatch equals the braid's weight (or the sum of both sides) by default — rejected. **`one` is the accepted default.** `weight` and `sum` remain as CLI variants for [issue #39](https://github.com/ramate-io/apa-kan/issues/39).
- Late-game double-move restriction (`--no-same-piece-twice when-deck-empty`) as the default — rejected. **`off` is the locked default** (doubles allowed) and the **only benchmarked** H2H configuration; `always` / `when-deck-empty` remain as CLI variants.
- Draw all three lanes even when a channel is quiet — rejected; omit quiet lanes.

## Open questions

- Whether a later visual pass should pack stacked diamonds more tightly on mobile.
- Whether `mismatch_uses_neighbor_weight` should ever apply to zag or failed-braid (today: normal channel only).
