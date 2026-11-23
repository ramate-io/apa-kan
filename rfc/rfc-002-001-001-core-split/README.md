# RFC 002-001-001: Extract `apa-kan-core`

Re-extracted against current `main` (locked base ruleset, PR #88) rather than cherry-picking #60's stale rule code. Original extract: PR #60.

## Problem

The workspace is one crate, `apa-kan` (`apa-kan/game`). `RuleSet`, `Game::legal_actions` / `Game::apply`, JSONL replay, bots, the PNG renderer, and the Bevy UI share that crate. `desktop` already gates Bevy, and `apa-kan-bench` runs without it, but there is no crate boundary. Server-side validation, a future lobby, and clocks-as-rules must not go through Bevy ([issue #48](https://github.com/ramate-io/apa-kan/issues/48), tracking [issue #58](https://github.com/ramate-io/apa-kan/issues/58)).

This is milestone **M0** of [issue #57](https://github.com/ramate-io/apa-kan/issues/57), plus the core clock types from M2 so local and later online play share one deterministic clock.

## Proposed solution

### Crate map (this RFC)

| Crate | Path | Owns | Must not depend on |
|---|---|---|---|
| `apa-kan-core` | `apa-kan/core` | `Board`, `Tile`, `RingType`, `CaptureMark`, `RuleSet`, `Action`, `Game`, `legal_actions` / `apply`, live locked scoring (additive braid-only-exact) plus historical reconstruction, deck/bench/reserves, beads, emblems, dot/triangle captures, local-claim / SWAP / CLAIM / CONVERT, bots, JSONL schema + deterministic replay, **clocks and flag-fall** | Bevy, sqlx, HTTP, `ml-dsa` |
| `apa-kan` | `apa-kan/game` | Bevy 0.19.1 client: board, draft, HUD, local bots as display, PNG renderer, CLI, character library. Thin re-export of core. Clock **display** only. | store, server, sqlx, Stripe, email |

Visual constants (`visual.rs`, `render.rs`) and the character catalog (`characters.rs`, `assets/characters/`) stay in the client, as [issue #48](https://github.com/ramate-io/apa-kan/issues/48) recommends. Scenario TOML files move with the loader to `apa-kan/core/scenarios/` because they are opening-deal data, not UI. Committed H2H JSONL stays under `apa-kan/game/benchmark_results/` (the bench binary still lives on the client crate). Bot TOML profiles stay under `apa-kan/game/bots/`.

Live `RuleSet::default` is the locked package from PR #88: ratchet reward, 2-2-1-1 opening, additive braid-only-exact, local-claim, normal-only SWAP. Extraction does not change those defaults. Historical JSONL still reconstructs older scoring / channel / endgame variants.

### Bots stay in core for M0

[Issue #48](https://github.com/ramate-io/apa-kan/issues/48) recommends a later `apa-kan-ai` crate so validators do not pull search. This RFC **keeps bots in `apa-kan-core`**: replay, `apa-kan-bench`, and the 7+4 picker already call `evaluate` / `Searcher`, and the bring-up task is to extract rules **and** bots without a behavior change. A follow-up can `git mv` `src/ai` to `apa-kan/ai` without touching `Game::apply`. Search knobs and default bot choice are out of scope here.

### Dependency direction

```
apa-kan-core          (no Bevy)
    ↑
apa-kan (Bevy client, optional `desktop` feature)
```

Arrows point toward the Bevy-free library. The Bevy crate never reaches a store or payment SDK. A future server depends on core, never on Bevy. Workspace `Cargo.toml` only adds the `apa-kan/core` member; other platform crates (`account`, `store`, `server`, from the #59 port) are out of scope here so parallel PRs can land them without a members-list fight.

Crate-root re-exports (`Game`, `Action`, `RuleSet`, `Clock`, `TimeControl`, …) are the intended API for those platform crates.

### Clocks (rules only)

[Issue #50](https://github.com/ramate-io/apa-kan/issues/50) **Decision:** ship **5-minute** and **2 + 1**. [Issue #58](https://github.com/ramate-io/apa-kan/issues/58) **Q21 Decision:** those labels are Fischer **5+0** and **2:00 + 1 s/move**.

| Label | Interpretation (**Decision**, #58 Q21) |
|---|---|
| 5-minute | Fischer **5+0**: 5:00 each, no increment |
| 2 + 1 | Fischer **2+1**: 2:00 each, **+1 second after every completed legal action** (ADD, MOVE, SWAP, CLAIM, CONVERT, or legal PASS) |

Core owns remaining milliseconds, increment, and flag-fall → `EndReason::Flag` / `FlagDraw`. It never reads a wall clock. The caller passes `elapsed_ms`. Unclocked games (`Game` with `clock: None`, today's default) are unchanged: `apply` does not touch time, and existing JSONL still replays.

```
Clock { remaining_ms[2], increment_ms, time_control }
Game::apply_elapsed(player, elapsed_ms)  // deduct; may flag
Game::credit_increment(player)           // after a completed action
Game::apply_timed(action, elapsed_ms)    // elapsed then apply then increment
```

Flag-fall is a clock outcome, not a board action, so search and historical logs do not grow a new `Action` kind.

Increment is credited only after a **successful** `apply`. An illegal action still spent the elapsed thinking time and gets no increment.

Using **exactly** the remaining milliseconds leaves 0 and is still a legal move; **exceeding** remaining flags. After a legal move, 2+1 adds 1000 ms (5+0 adds 0).

**Flag-fall (OPEN-Q #23, issue Proposal):** default is a loss for the flagged player (`EndReason::Flag`). Draw (`EndReason::FlagDraw`) when the opponent cannot possibly win under cheap checks, not a search:

1. opponent is not alive (no board tiles, empty remaining bench), or
2. opponent would already lose by lockout (`no_moves_loses` and no legal ADD/MOVE/SWAP/CLAIM/CONVERT on their turn), or
3. both sides are already in the low-value pair-check and the opponent would lose or draw that check.

These sit behind `Game::flag_opponent_cannot_win` so "flag is always a loss" is a one-function change.

**Disconnect (#58 Q26 Decision):** the clock does **not** pause during a reconnect window. Core never pauses; a 60 s reconnect is a caller/server choice of which `elapsed_ms` to pass. Not implemented here.

**Ratings and seasons (not stored in this crate):** recorded so a later `apa-kan-rating` crate does not fight core.

- **Q1 Decision:** one Elo ladder for now, not split by tile option or deck option.
- **Q2 Decision:** wagered games **do** write Elo.
- **Divisions / seasons Decision:** Elo-per-game percentiles are confined to games within each mode and kept separate for human and bot games (a human division is distinct from a bot division). Starting-Elo bands are approved.
- **Q22** (split the ladder by time control) is unanswered and unused here. `TimeControl` is a match-clock axis, not a rating key.

Draft clocks (60 s / 45 s / 60 s) are a later lobby concern and do not consume match time.

JSONL: optional `time_control` on the game and optional `elapsed_ms` on each action, both `skip_serializing_if` absent. Logs without them replay through `apply` exactly as today. A log that *has* `time_control` but no per-action `elapsed_ms` is rejected.

### Behavior

No rules change to the locked defaults, overwhelm, beads, emblems, capture marks, 7+4, SWAP, CLAIM, CONVERT, or ratchet/opening. Proof:

- existing unit tests move with the modules into `apa-kan-core`
- committed H2H JSONL under `apa-kan/game/benchmark_results/` (including historical `zag_color_exclusion` and live `opening_2211`) still replays through core to the same terminals

`apa-kan-bench` remains a bin of `apa-kan` that uses core without the `desktop` feature.

## Implementation details

- New crate `apa-kan/core` (`package = "apa-kan-core"`)
- Moved from `apa-kan/game/src/`: `board.rs`, `game.rs`, `deck.rs`, `scenario.rs`, `bench_log.rs`, `ai/`
- Moved `apa-kan/game/scenarios/` → `apa-kan/core/scenarios/`
- New `apa-kan/core/src/clock.rs`
- `apa-kan/game` depends on `apa-kan-core` and re-exports `ai`, `bench_log`, `board`, `clock`, `deck`, `game`, `scenario` so in-crate `use crate::game::…` in the UI still works
- Character library, Bevy UI, PNG renderer, CLI, and bot TOML stay in `apa-kan/game`
- Optional `test-helpers` feature exposes `#[doc(hidden)]` cascade HUD fixtures so the client crate's tests can still build positions that used to be `pub(crate)` in the same crate
- CI: `.github/workflows/cargo-core.yml` runs `cargo test -p apa-kan-core` and asserts `cargo tree -p apa-kan-core` has no `bevy`

## Alternatives considered

- Split `apa-kan-ai` in the same PR — rejected for M0. Picker + replay + bench share one crate; a later `git mv` is the issue's option.
- Bake clocks into `RuleSet` — rejected. Time control is a match axis, not an overwhelm knob. Default games stay unclocked.
- `Action::Flag` — rejected. Flag is not a legal board action; search must not see it. End reason + `apply_elapsed` is enough to replay a flag ply.
- `no_std` on day one — rejected. `alloc` + `std` (rand, flate2, serde) matches today's modules. `no_std`-capable later if it is cheap.
- Move `visual` / `render` / `characters` into core or `apa-kan-view` — rejected. Painters and Bevy asset catalogs are client; they do not block extraction.

## Decisions from #58 (this RFC)

| # | Decision |
|---|---|
| **21** | 5-minute = Fischer **5+0**; **2 + 1** = 2:00 + **1 s increment per move**. Constants in `clock.rs`. |
| **26** | Reconnect does **not** pause the clock. `apply_elapsed` always deducts the given `elapsed_ms`. |
| **1** | One Elo ladder for now, not split by tile or deck option. No rating types in this crate. |
| **2** | Wagered games **do** affect Elo. Rating storage is `apa-kan-rating`, not core. |
| Divisions / seasons | Elo-per-game percentiles stay inside each mode and are separate for human vs bot games. Starting-Elo bands are approved. Not stored here. |

## Remaining open question

| # | Question | Assumed in this RFC |
|---|---|---|
| **23** | Flag-fall always a loss, or draw when the opponent cannot win (cheap checks)? | **Draw** in those cases (`EndReason::FlagDraw`), issue #50 recommendation. Lives in `Game::flag_opponent_cannot_win` so "always a loss" is a one-function change. Marked `// OPEN-Q #23`. |

Q22 (ladder split by time control) is unanswered; core does not rate games.

Server-authoritative clocks, signed `ClockEvent` envelopes, and lag budget remain [`rfc-002-003-002`](https://github.com/ramate-io/apa-kan/issues/57) / M3.
