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
- **Doubles allowed:** `--no-same-piece-twice off`. `always` and `when-deck-empty` remain as non-default variants.

Supported by the committed [`doubles_allowed`](../../apa-kan/game/benchmark_results/doubles_allowed.txt) benchmark (12 games/pairing/seat, 1000 ms, seed 1): P1 **46-18-44**, mean avg turns **68.51**.

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
- `BraidFailMismatch` on `RuleSet` (`--braid-fail-mismatch one|weight|sum`)
- `ArcPaint.braid: Option<BraidLane>` plus lane offsets in `visual.rs`; renderers in `render.rs` / `ui.rs`
- JSONL writes `braid_fail_mismatch`. Historical logs omit it; full replay of those games is skipped because captures used the old overrides.

## Alternatives considered

- Keep zag-hits replacing the ordinary mismatch — rejected; a same-color zag vs normal should net 0.
- Failed-braid mismatch equals the braid's weight (or the sum of both sides) by default — rejected. **`one` is the accepted default.** `weight` and `sum` remain as CLI variants for [issue #39](https://github.com/ramate-io/apa-kan/issues/39).
- Late-game double-move restriction (`--no-same-piece-twice when-deck-empty`) as the default — rejected. **`off` is the accepted default** (doubles allowed); `always` / `when-deck-empty` remain as CLI variants.
- Draw all three lanes even when a channel is quiet — rejected; omit quiet lanes.

## Open questions

- Whether a later visual pass should pack stacked diamonds more tightly on mobile.
- Whether `mismatch_uses_neighbor_weight` should ever apply to zag or failed-braid (today: normal channel only).
