# RFC 001-001-005: Range modifier

## Problem

Adjacent-only channels cannot punish or support a tile that sits one hop away with an empty cell in between. Range is a **per-ring modifier** (not a fifth ring type) that extends that ring’s channel effects along **uninterrupted second-order** connections.

## Proposed solution

Each live ring independently gains range with chance `range_numer / 7` (default 1), rolled **after** braid, zag, and bead so `--range off` / `--range-chance 0` skips the RNG and matches the pre-range deal for the same seed. Range can sit on a normal, zag, braid, or bead ring.

Range does **not** change weight (`weight == n_rings`), does **not** count toward the 6-per-type bench cap (it is not a type), and does **not** enable pass-through or hops.

### Uninterrupted 2-arc walks

A ranged connection from occupied node `A` to occupied node `B` is a walk `A — mid — B` of **two generating-circle arcs** where `mid` is **empty**.

- An occupied immediate neighbor **blocks** every 2-arc walk through that cell. Reach only goes through an empty intermediate.
- Walks respect the board graph, including inner↔center **double arcs**.
- Multiple distinct paths count independently. Scoring walks `Board::incident_others` at each step, so the cartesian product of consecutive incident arcs is the multiplicity. Inner through an empty center to another inner is **2 × 2 = 4** ranged connections, matching the per-petal adjacent double-edge rule.
- A tile that is both a unique neighbor of `A` (adjacent) **and** reachable by a 2-arc walk is scored twice: once on the adjacent edge (full channels) and once per ranged walk (ranged channels only).

### Ranged-edge scoring (locked in)

Each 2-arc walk is an undirected edge. **Both tiles are charged** the same totals (`ranged_edge_channels(a, b) == ranged_edge_channels(b, a)`).

Only rings that carry the range modifier **act**. The facing tile presents its **full** color set / ring multiset (ranged or not) as the hit / fulfillment / presence test.

| Channel | Fires at range when | Match | Mismatch |
|---|---|---|---|
| **Normal** | a **ranged** normal ring on either tile | 1, if some ranged-normal color of either tile is in the other’s full color set | 1 hitch otherwise |
| **Zag** | a **ranged** zag on either tile | never | `zag_mismatch` counting only ranged zag rings; hit test is the facing tile’s full colors |
| **Braid** | a braid **extends** (every braid slot on that tile is ranged) | slot count of the selected extending fulfilled braid | `braid_fail_mismatch` over extending braids only |
| **Bead** | a **ranged** bead on either tile | 1 if any ranged bead is matched (`other.has_color(C)`), **one −1 per ranged edge**, not per bead | +1 per unmatched **ranged** bead ring |

Unranged rings on either side do not fire at range. Adjacent scoring is unchanged and still uses `edge_channels` (all rings).

Ranged channel totals **add** into the same `tile_effective_weight` / `tile_score` used by overwhelm. Simultaneous capture still snapshots every occupied node after the action; ranged edges are included in that snapshot.

### Braid, bead, zag, overwhelm

- **Braid:** the braid effect extends iff **all** braid slots are ranged. A 2-braid with only one ranged thread does not send the braid channel to second-order cells; it still scores adjacent as today. Fulfillment at range still reads the neighbor’s full ring-color multiset.
- **Bead match cap:** still one −1 at most, **per ranged edge** (each 2-arc walk). Three ranged orange beads vs one orange is match 1 on that walk, not 3.
- **Zag:** only ranged zag rings count in `count_ranged`. Non-ranged zags stay adjacent-only.
- **Overwhelm / simultaneous capture:** ranged hitch and match add to the same signed total as adjacent edges. Both ends of a ranged walk take the total, so mutual overwhelm is possible and both removals land in the same `collect_removals` pass.

### Generation, cap, weight

Default 1/7 per live ring, any type, after braid/zag/bead. `--range off` is `range_numer = 0`.

The 6-per-type cap on the 11-tile bench is still zag / braid / bead ring slots only. Range is a modifier, not a type, so it is **not** capped unless a later rule says otherwise.

### Visual

Open circles on the **four antipodes** of the ranged ring itself (east, north, west, south of that concentric stroke). Braids that extend draw the same marks on the braid rim. Existing lane paint, bead discs, zag teeth, failed-braid diamonds, token stacks, and “deck row = unplaced bench only” are unchanged. Second-order connections are **not** drawn as extra arcs.

## Worked cases

Signs: match is a positive match-weight; the signed edge is `mismatch − match`. Multiplicity is the number of 2-arc walks.

| Case | Walks | Per walk | Notes |
|---|---|---|---|
| Ranged orange normal vs blue, empty center, two inners | 4 | hitch 1 | both tiles +4 |
| Occupied center | 0 | — | blocked |
| Unranged orange + ranged green vs orange | 1 | hitch 1 | the unranged orange does **not** match at range; adjacent still matches |
| 3 ranged orange beads vs 1 orange | 1 | bead match 1 | cap is per ranged edge |
| 2-braid, only one slot ranged, vs covering neighbor | 1 | braid silent | `braid_extends` is false |
| 2-braid, both slots ranged, vs covering neighbor | 1 | braid match 2 | fulfillment uses full facing colors |

Unit tests in `game::tests` (`occupied_intermediate_blocks_second_order_reach`, `empty_intermediate_reaches_second_order`, `center_empty_inner_to_inner_is_four_ranged_walks`, `unranged_rings_do_not_act_at_range`, `braid_extends_only_when_every_slot_is_ranged`, `bead_match_cap_is_per_ranged_edge`, `simultaneous_capture_includes_ranged_edges`, …) lock these. `--jobs 1` vs `--jobs 4` JSONL equality stays in `apa-kan-bench`.

## Implementation

- `Tile::ring_range`, `Tile::with_ranged_rings`, `Game::ranged_edge_channels`, `Game::for_each_ranged_other` in `apa-kan/game/src/game.rs`
- `--range-chance` / `--range`; `RuleSet::range_numer` (default 1)
- Kind index does **not** include range (`COLOR_STATES` stays type×color). Search packs a 3-bit range mask into `SearchMove` / TT `b` and Zobrist has separate range tables.
- JSONL: ring flag `r`, `range_rings`, `range_numer` (absent/0 on older logs; replay then has range off)
- Scenario TOML: `range = true` (all slots) or `ranges = ["orange"]`
- Visual: `visual::range_antipodes` / `TILE_RANGE_R` open circles on the ring

## Alternatives considered

- Range as a fifth ring type — rejected. Confirmed: modifier on any ring.
- Reach through occupied tiles (or “line of sight” that skips occupants) — rejected. Confirmed: empty intermediate only.
- Collapse multiple walks to the same dest into one edge — rejected. Consistent with per-arc double-edge scoring: cartesian product of incident arcs.
- Only the ranged tile is charged — rejected. Adjacent edges are undirected and both-charged; ranged walks follow that.
- Braid extends if **any** thread is ranged — rejected. All slots must be ranged, otherwise a single ranged thread would bomb second-order cells with a 2- or 3-match.
- Bead match cap shared across all walks to the same dest — rejected. Each walk is a separate edge, like center petals.
- Range counts toward the 6-per-type cap or adds weight — rejected. No strong reason; it is not a type.

## Open questions (flagged for the user)

These are implemented as above. Confirm or override:

1. **Which rings participate / both charged.** Only ranged rings act; the facing tile presents full colors; both tiles take the undirected total. Alternative: charge only the ranged side, or project both tiles’ full rings at range.
2. **Braid fulfillment at range.** Extends iff every braid slot is ranged; fulfillment still uses the neighbor’s full multiset. Alternative: any ranged braid slot extends, or fulfillment also requires ranged colors on the facing tile.
3. **Bead single-match cap.** One −1 per ranged 2-arc walk. Alternative: one −1 per dest node, collapsing walks.
4. **Zag / overwhelm / simultaneous capture.** Ranged zags only; ranged totals add to the same snapshot as adjacent; mutual capture is allowed. Alternative: ranged mismatch never overwhelms on the same pass as adjacent.
5. **Weight and 6-per-type cap.** Unchanged. Alternative: cap ranged rings separately, or treat range as a type.
6. **Visual second-order ink.** Tile-rim open circles only; no extra arcs to second-order cells. Alternative: ghost lanes along each 2-arc walk.
7. **Triangle (adjacent + ranged to the same dest).** Both scores add. Alternative: ranged walks skip unique neighbors.
