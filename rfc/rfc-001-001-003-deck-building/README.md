# RFC 001-001-003: Deck-building with bench and reserves

## Problem

The accepted default after [`rfc-001-001-002`](../rfc-001-001-002-independent-channels/README.md) dealt each player a public 11-tile deck. There was no collection behind that hand, no draft, and no per-type cap on special rings. This RFC restores a 27-tile collection and a 7+4 public bench without changing ADD, usable budget, or spawn tokens.

## Proposed solution

### Collection, bench, reserves

Each player has a **collection** of at most 27 tiles (`RuleSet::deck_size`, default 27; CLI `--deck-size` / `--bag-size`). Setup is:

1. Both players secretly pick a **bench of 7** from their collection. The leftover 20 tiles are **reserves**.
2. Both **reveal** the 7.
3. Each player picks **4 more** from their own reserves, seeing the opponent's revealed 7.
4. The resulting **11-tile public bench** replaces today's 11-tile public deck for the rest of the game.

Reserves stay in `Game::bags` and are **never usable in play** (not ADD-able, not part of usable budget, not shown on the deck row). `RuleSet::bag_size` remains the public bench size (default 11) and is synced to the drafted bench after setup.

ADD, usable budget, and spawn tokens are unchanged:

- usable budget = `min(tokens, unplaced bench tiles)`
- 7 starting tokens (`--starting-tokens` / `--starting-bench`)
- +1 token per capture (self-capture still gated by `self_capture_budget`)
- the full 11 public tiles are selectable once chosen

### Ring-type cap (confirmed)

The full 11-tile bench may have at most **6 rings of each non-normal type** (zag, braid, and bead). **Normal rings are uncapped.**

**Braids count by ring slots toward that cap.** A 3-braid uses 3 of the 6. Two 3-braids use 6 of 6 and are legal; a third braid of any size is illegal. This is a confirmed rule, not an assumption.

The cap is enforced at both the 7-pick and the +4 extension: a set that would exceed the cap on the eventual 11 (or on the 7 alone) is illegal. The UI shows `zag n/6 braid n/6 bead n/6` and refuses an illegal click. AI pickers never emit an illegal set.

### Generation and picking

Default collections are sampled from the ring/color/zag/braid/bead vocabulary (`sample_tile`). Mirrored collections (`identical_bags`, default true) copy P1's kinds onto P2 with new ids. `--deck-noise 0..100` independently resamples that percent of the mirrored copy. Generation retries until a 11-subset under the cap exists, then repairs leftover specials to normal if needed.

Picking is **never uniform-random**. `--pick heuristic|search|greedy`:

- **greedy**: rank tiles, take the first legal set that leaves room for the later +4
- **heuristic** (default): greedy, then hill-climb under the work budget
- **search**: heuristic plus short seeded rollouts on the top candidates

Work budget is the same `--budget nodes:...,iters:...` the H2H searcher uses (`apa-kan-bench` copies it onto the picker). Deterministic per `(deck_seed, pick, budget)`. `--deck-seed` defaults to `--seed`. `--jobs 1` and `--jobs 4` with the same seed produce identical JSONL.

Small scenario files (`collection length ≤ 11`) keep listed order (7+4 split of the listed tiles) so 2–3 tile fixtures are not reordered by the picker. Larger file scenarios (complementary's 18) draft 7+4 from the listed collection.

### Logging

JSONL `v=1` gains an optional `setup` object: both collections, both 7-picks, both +4s, `deck_seed`, `deck_noise`, and `pick`. `opening.p*_bench` is the public 11; `opening.p*_bag` is the leftover reserves (never usable). Logs from before this RFC omit `setup` and still replay from the opening benches.

### UI

Humans walk the 7-pick, reveal, and +4 on a collection grid. The cap is shown and enforced. Bots and screenshots auto-complete the draft. In-game visuals are unchanged: lanes, solid tokens, deck row = unplaced **bench** only.

## Implementation details

- `apa-kan/game/src/deck.rs` — generation, cap, greedy / hill-climb / rollouts, `SetupRecord`
- `RuleSet::deck_size` (27) beside `RuleSet::bag_size` (11 public bench)
- `Game::start_with` / `from_setup_record` / `from_scenario_with`
- CLI: `--deck-seed`, `--deck-noise`, `--pick`; `--deck-size` / `--bag-size` set collection size
- `SetupLog` on `GameLog`

## Alternatives considered

- Cap by tile count (6 zag *tiles*) — rejected. The confirmed rule is ring slots.
- Count a braid as 1 toward the cap regardless of size — rejected. A 3-braid uses 3 of 6.
- Keep reserves ADD-able or drawn on capture — rejected. Reserves are out of the game.
- Uniform-random 7+4 — rejected. Picks are intelligence (heuristic/search) and seeded.

## Open questions

- Range modifier is a later PR and is not part of this draft. Beads are specified in [`rfc-001-001-004`](../rfc-001-001-004-beads/README.md) and share this 6-slot cap.
