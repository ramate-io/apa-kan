# RFC 001-001-005: Dot and triangle capture modifiers

## Problem

Every capture used to become a spawn token. That couples board fights to ADD permission: a player who captures well always gets more tokens, never more tiles to place. Range (an earlier draft of this slot) was dropped as too hard to calculate. Capture destination is a simpler per-tile modifier that still changes what winning a fight is worth.

## Proposed solution

Every tile carries a **capture mark**: a **dot** or a **triangle**. These replace the move-length dots drawn on the tile. There is no unmarked tile.

- The **number** of marks is `Tile.dots` (MOVE length 1–3). Move length still varies per tile.
- The **shape** is the capture destination of a move or ADD **made with that tile**.

### Destination

“Move made with” = the tile that was MOVEd or ADDed this turn.

| Mark | Captures from that action |
|---|---|
| **Dot** | Every captured tile (opponent or the mover's own via self-capture) joins the **mover's bench**, owner = mover, as an unplaced tile they can ADD later with a spawn token. |
| **Triangle** | +1 **spawn token** per captured tile (today's behavior). Self-capture tokens still gated by `self_capture_budget`. |

Self-captures follow the same dest. A single move's simultaneous snapshot is unchanged; every removal from that snapshot goes to **one** destination.

### Caps and budget

Tiles captured to the bench **may** push it past 11 tiles and past the 6-per-type ring cap. That cap applies only at 7+4 bench **selection**, not to captured tiles. Usable budget remains `min(tokens, unplaced bench tiles)` and the low-value check uses the live bench.

### Generation and picking

`sample_tile` rolls `dot_percent` (default 50, CLI `--dot-chance 0..100`): that percent of generated tiles are dots, the rest triangles. Scenario TOML may set `capture = "dot"` / `"triangle"`; omitted listed tiles default to triangle so old files keep token captures.

The 7+4 picker scores capture-mark mix. All-dot benches starve spawn tokens; all-triangle benches starve later ADDs once the opening hand is spent. Slightly triangle-heavy (~45% dots) is preferred.

### Visual, search, logs

Filled circles vs filled up-pointing triangles, 1–3 of them, same primary-color fill as today's dots. The deck row lists every live bench tile (opening-deal order, then captured extras), so opponent tiles that flipped onto the mover's bench are selectable there. Triangle captures become spawn tokens on the mover's stack, not hand tiles. The freeze-frame for a bench dest shows the flipped (mover-owned) tiles; a token dest keeps the captured face.

Search packs the mark as an extra bit (`kind | (mark << 16)`, TT `b = dots | (mark << 2)`). `KIND_COUNT` is not doubled (that overflows `u16`). Zobrist XORs a mark table. Eval penalizes starved-token / starved-bench mixes.

JSONL: each tile has `capture` (`dot`/`triangle`; absent on older logs = triangle). Each action with removals has `capture_dest` and a `captures` list with dest per removal.

## Worked cases

- Triangle MOVE that removes an opponent tile: +1 token, bench unchanged.
- Dot MOVE that removes an opponent tile: that tile appears on the mover's bench with mover ownership; tokens unchanged.
- Dot self-capture: own tile leaves the board and returns to the mover's bench (still owned by the mover).
- One MOVE that removes both an opponent tile and an own tile: both go to the same dest.

## Implementation

- `CaptureMark`, `CaptureDest`, `Tile.capture`, `RuleSet.dot_percent` in `apa-kan/game/src/game.rs`
- `--dot-chance`; generation 50/50; picker mix in `deck.rs`
- Search/TT/Zobrist extra bit; eval mix score
- JSONL `capture` / `capture_dest` / `captures`
- Visual n-circles vs n-triangles in `ui.rs` / `render.rs`
- Tests: dest, self-capture ownership, mixed snapshot, bench overflow, `--jobs 1` vs `--jobs 4`

## Alternatives considered

- Range as a per-ring modifier (dropped: too hard to calculate).
- Kind-space doubling for the mark: `KIND_COUNT * 2` does not fit `u16`.
- Unmarked tiles: rejected; every tile has a mark.
- Cap captured bench tiles: rejected; cap is selection-only.

## Open questions (resolved here, not blocking)

1. **Move length.** `dots` is 1–3 exact MOVE steps. Preserved: number of marks = move length, shape = dest.
2. **Bench overflow.** Captured tiles may exceed 11 and the 6-per-type cap. Usable budget and low-value use the live bench.
3. **Simultaneous snapshot.** Unchanged. One dest per action. Captured-to-bench tiles change `owner` to the mover.

## H2H

Committed [`capture_modifiers`](../../apa-kan/game/benchmark_results/capture_modifiers) (release, `--budget nodes:132000,iters:2600`, `--jobs 4`, 12/pairing/seat, seed 1, `--no-depth-probe`, `--pick search`, 27-tile collections). P1 **49-7-52**, ends 41/18/2/47/0, mean avg turns **39.63**. Vs PR #40 [`work_budget_doubles_allowed`](../../apa-kan/game/benchmark_results/work_budget_doubles_allowed) (**45-17-46**, 38/7/14/49/0, **60.29**). Capture dest 507 bench / 632 tokens. Picker mostly 5 dots of 11 (46-7-45); 6-dot openings 1-0-5.
