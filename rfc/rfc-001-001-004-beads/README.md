# RFC 001-001-004: Beads ring type

## Problem

Zags punish a **matching** color (a countable hit on one particular color). Highly connected **braided** pieces often share many colors with their neighbors, so a zag of one color is a weak answer: it may wash against a leftover normal, and it never bombs the rest of the braid. This RFC adds a fourth independent ring type, **beads**, as a bomb against **non-matching** colors — especially as a counter to braids whose colors the beads do not hold.

## Proposed solution

Beads are like zags in generation and weight (per-ring, weight 1), but the **bead channel** inverts the color test: unmatched bead rings punish, and a match is a single −1 like the normal channel.

### Matching (confirmed)

A bead ring of color `C` on tile `T` is **matched** by neighbor `N` iff `N.has_non_zag_color(C)`: `C` appears among `N`'s live **non-zag** rings (normal, braid, or bead). Zag rings never count as a bead match.

This is the same **presence** test the normal channel uses for a normal ring (`Tile::normal_color_in` / `other.has_non_zag_color(c)`). It is **not**:

- multiset coverage (three orange beads vs one orange = all three matched)
- the edge-level OR of the normal channel (a bead does not become matched just because some other normal ring on the edge matches)
- a per-color-pair count

Documented here as the chosen reading of “the same notion of a color matching on the neighbor that the normal channel uses.”

### Bead channel

The channel fires only if at least one tile on the edge has a bead ring. Zag and braid still add. Default **shared-channel** scoring (rule B) takes `max(normal, bead)` per match and per mismatch instead of summing those two channels. `--bead-scoring independent` restores the additive rule A.

| | Match | Mismatch |
|---|---|---|
| **Bead** | 1, if **any** bead ring on either tile is matched | +1 per **unmatched bead ring** on either tile |

- Count unmatched **bead rings**, not each mismatching color pair. Three unmatched orange beads vs blue is +3, not 3×(blue,green,…) pairs.
- If any bead is matched, the channel also scores **one** match of 1 (one −1 at most per edge, like normals — not per bead).
- Mixed edges can score both: orange+green beads vs orange-only is match 1 + unmatched 1.
- Both tiles take the same undirected totals (`bead_channel(a,b) == bead_channel(b,a)`).
- Center double edges count **per arc** as today: each inner↔center petal is a separate edge.
- Weight: every bead ring weighs 1. `ring_weight` is still the live ring-slot count.

Beads do **not** enable pass-through or hops (same as zags). Low-value: beads count 1 on the primary pass and 2 on the tie-break, like zag and braid.

### Intended consequences

- Same-color bead vs normal: normal match 1 + bead match 1.
- Same-color bead vs zag: zag hit 1 + unmatched bead 1 (the zag is not a match).
- Three orange beads vs a blue+teal+green braid: bead +3 + failed-braid +1. A 3-ring braid is overwhelmed. That is the bomb against a highly connected braid of non-matching colors.
- Three orange beads vs an orange+green braid: all three orange beads are matched (presence) → bead match 1, unmatched 0; the braid still needs green, so it may fail on its own channel.

### Generation and cap

Under `--channel-mode additive`, a piece forms a **2–3 bead group** with chance `bead_numer / 7` (default 1), rolled after braid and zag, occupying remaining **normal** slots (padding a short tile by repeating its last color, like braids). Generated tiles never have a single bead ring.

The experimental default (`--channel-mode independent-max`, [`rfc-001-001-008`](../rfc-001-001-008-independent-max-channels/README.md)) elects beads per light slot with normal:zag:bead = 5:3:1. Single-bead tiles are allowed; custom TOML may list one bead. `--bead-chance 0` / `--beads off` drops the bead weight. Scoring accepts 1-bead fixtures in both modes.

Beads count toward the **6-per-type cap** on the 11-tile bench (6 bead rings max), alongside zag and braid, counted by ring slots. Normal stays uncapped. Enforced at the 7-pick and the +4.

### Visual

Tile rims: a connected swelling/pinching ribbon per bead ring (2 or 3 concentric chains on a generated tile), distinct from solid normals, zigzag zags, and braid weaves.

Lanes, fixed order: **normal, zag, braid, bead**. Quiet channels omitted. 1–3 active lanes keep today's offsets; four use `±1.5 pitch` and `±0.5 pitch`. Failed-braid diamonds, solid token stacks, and “deck row = unplaced bench only” are unchanged.

Setup HUD: `zag n/6 braid n/6 bead n/6`.

## Worked table

Signs: match is a positive match-weight; the signed edge is `mismatch − match`.

| Case | A | B | Normal | Zag | Braid | Bead | Edge `(m, x)` | signed |
|---|---|---|---|---|---|---|---|---|
| 3 orange beads vs blue | 3 orange beads | blue normal | hitch 1 | — | — | unmatched 3 | (0, 4) | +4 |
| 3 orange beads vs 1 orange | 3 orange beads | orange normal | match 1 | — | — | match 1 | (2, 0) | −2 |
| mixed orange+green vs orange | orange+green beads | orange normal | match 1 | — | — | match 1 + unmatched 1 | (2, 1) | −1 |
| bead vs same-color zag | orange bead | orange zag | — | +1 | — | unmatched 1 | (0, 2) | +2 |
| beads vs non-matching 3-braid | 3 orange beads | blue+teal+green braid | — | — | fail 1 | unmatched 3 | (0, 4) | +4 |

Unit tests in `game::tests` (`bead_unmatched_counts_rings_not_color_pairs`, `bead_match_is_capped_at_one_per_edge`, `center_inner_bead_mismatch_scores_per_petal`, …) lock these.

## Implementation

- `RingType::Bead`, `Tile::bead_channel`, `EdgeChannels::{bead_match,bead_mismatch}` in `apa-kan/game/src/game.rs`
- `--bead-chance` / `--beads`; `RuleSet::bead_numer` (default 1)
- Cap DP in `deck.rs` is 3-dimensional (zag, braid, bead)
- JSONL: ring flag `e`, `bead_rings`, `bead_numer` (absent/0 on older logs), `bead_scoring`
- Visual: `bead_ring_centers` / bead lane in `visual.rs`, `ui.rs`, `render.rs`
- CLI: `--bead-scoring independent|shared` (default **shared**)

### Shared-channel default (rule B)

`--bead-scoring shared` is the live default. It keeps the same bead matching rule (presence, −1 at most once) but **does not add** the bead channel to the normal channel. On each edge the normal/bead contribution is `max(normal match, bead match)` and `max(normal mismatch, bead mismatch)`. Zag and braid still add. `--bead-scoring independent` restores rule A (sum). JSONL logs that omit `bead_scoring` still replay as independent.

## Alternatives considered

- Count each mismatching color pair (3 beads × 3 braid colors = 9) — rejected. Confirmed: unmatched **bead rings**.
- Match −1 per matched bead — rejected. One −1 at most per edge, like normals.
- Multiset coverage (orange+orange beads need two oranges) — rejected. Same presence test as the normal channel.
- Per-ring independent beads (like zags) — restored as the experimental default (`independent-max` light election). Additive mode still generates 2–3 groups only.

## Open questions

- Capture-mark destination (dot → bench, triangle → tokens) is a later PR and is not a ring type. See [`rfc-001-001-005`](../rfc-001-001-005-capture-modifiers/README.md).
