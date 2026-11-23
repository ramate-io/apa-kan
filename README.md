# Apa Kan

A turn-based game of colors and adjacency on an 8-petal rosette, built with Rust and Bevy 0.19.1.

Rules, bots, JSONL replay, and match clocks live in `apa-kan-core` (`apa-kan/core`). The `apa-kan` crate is the Bevy client (display and in-match UI) and depends on core; core does not depend on Bevy. Default games stay unclocked. Optional Fischer 5+0 / 2+1 clocks are a match axis, not a `RuleSet` knob.

## How to play

The board is a 25-node rosette: a **center**, then three rings of eight (inner, spawn, outer). The eight yellow nodes are **spawns** — that is where new tiles enter. No two spawns touch.

**Setup.** Both players start from the same 27-tile collection. Draft an 11-tile public bench: pick 7 in secret, reveal, then pick 4 more from your reserves with the opponent's 7 in view. Leftover reserves are out of the game. You also start with 7 spawn tokens. Each tile has 1–3 colored rings (its **weight** is that ring count). A ring is normal, zag (never matches), bead, or a woven **braid**. A lone heavy on a 1-ring tile is a one-slot braid.

**Opening and turn order.** Player 1 takes two actions, then Player 2 takes two, then you alternate one action at a time.

**On your action** choose one:

- **Add** a bench tile onto an empty spawn. Costs one token.
- **Move** one of your tiles exactly as many steps as its marks. You may pass through an occupied node that shares a normal color if that occupant has at least as many rings as you. You cannot reuse an edge on the path.
- **Swap** a one-ring **normal** with an adjacent occupied tile (yours or theirs). The same pair cannot swap back on the very next action.
- **Claim** any currently overwhelmed tile, yours or theirs. It leaves the board and you gain a token. No token cost.
- **Flip** (only after the second ratchet — see below): turn an opponent's tile to your side. It stays where it is.

Add, move, swap, and flip capture **locally**: the landing tile and its neighbors, if they are overwhelmed. Other overwhelmed tiles sit until someone claims them. Captures from a **dot** tile join your bench; captures from a **triangle** become spawn tokens.

**Scoring and braids.** Neighboring tiles score by color channels — normal, zag, bead, braid — and those channels add. A zag never matches. Beads are their own channel. Only an **exact** braid match scores (a one-slot braid scores +1 when the neighbor has exactly that braid color once). Other matching rings still paint and occupy the space, but they score 0. The two petals between the center and an inner-ring node each count, so that link is scored twice. A tile is **overwhelmed** when its mismatch-minus-match pressure is at least its ring weight.

**The center and rings.** The **control set** is the center plus the eight inner-ring nodes (nine in all). Occupying enough of them wins. The target starts at 5 and can only drop — to 4, then 3, then 2 — as the larger side's material (tiles on the board plus usable tokens) falls. It never rises again. Holding the current target for one of your turns wins. If an action drops the target onto a number that exactly one player already occupies, that player wins immediately.

**Ratchets.** A ratchet fires when the center piece is captured, or when the control target drops without that immediate unique-holder win. The winner is whoever had more material going into the action (then more spawn tokens). A tie awards nothing.

The next action always belongs to the ratchet winner. If the opponent triggered it, that is simply their normal turn with the reward; if you triggered it, you go again. The reward lasts for that one action only. Add and claim stay legal.

- After the **first** ratchet of the game, the winner may **move or swap**. That swap may be any of their pieces with any occupied tile.
- After the **second** ratchet overall (whoever caused it), the winner may **flip, swap, or move** — the same loosened swap, and the flip can target any opponent piece.

**How the game ends.** You win by occupying the current control target, by eliminating the opponent (nothing on the board and nothing left on their bench), or by **lockout**: if the player to move has no legal add, move, swap, claim, or flip, they lose. When both sides are down to at most one piece of material, the lowest remaining pieces decide. Turn 200 is a draw.

## Locked defaults

Live `RuleSet`, CLI, bots, and UI share this package. `--cascade-winner-swap off` and `--opening 1-1-1-1` are the comparison toggles; other experiment flags were removed.

- 2-2-1-1 opening; ratchet reward on (cascade drops count; flips anywhere after the second ratchet)
- Additive channels, braid-only-exact, single braids on, beads separate
- Center↔inner links double-counted; ring weights 4:3:1; color-drift 160
- Same 27-tile start; adjacent normal-only swap with no immediate swap-back
- Local-claim captures; control-cascade endgame; doubles allowed

JSONL missing `cascade_winner_swap` / `opening_double` still replays those logs as off. Engine enums for old scoring, hold-win, and channel variants remain so historical logs reconstruct.

## Run

```bash
nix develop
cargo run -p apa-kan
```

iOS (Mac, unsigned Simulator): [`packaging/ios/README.md`](packaging/ios/README.md). Bundle id `io.ramate.apakan`.

A plain `cargo run --no-default-features -p apa-kan` is the playtest package (human vs greedy). With a desktop window: `nix develop && cargo run -p apa-kan`. Hot-seat: `--bot1 human --bot2 human`. Watch two bots: `--bot1 mcts --bot2 greedy`. Live search is `--search-profile enhanced`. `--search-profile exactplay` is the same tree with `eval_margin_scale=200` (hillclimbed vs enhanced; [`exactplay`](apa-kan/game/bots/exactplay.toml)).

```text
Usage: apa-kan [OPTIONS]
  --bot1 / --bot2 <human|random|greedy|minimax|minimax-legacy|mcts>
  --search-profile <enhanced|classic|plain|tactical|positional|explorative|deep_mcts|counterplay|exactplay>
  --bot-config <path.toml>                   Overlay a SearchConfig TOML
  --seed / --deck-seed <u64>
  --same-deck <on|off>                       Identical 27-tile start (default: on)
  --think-ms / --delay-ms <n>
  --cascade-winner-swap <on|off>             Ratchet reward (default: on)
  --ratchet-cascade <on|off>                 Cascade drops count as ratchets (default: on)
  --opening-double <on|off> / --opening <2-2-1-1|1-1-1-1>
  --cascade-turn-gain <on|off>               Extra-turn on cascade drop when the ratchet is off (default: on)
  --endgame-rule <control-cascade|low-value>
  --capture-mode <local-claim|cascade>
  --singles-swap <on|off|normal-only>
  --single-braids <on|off>
  -h, --help
```

`apa-kan --help` lists generation knobs (ring weights, zags, beads, emblems, drift, deck size, tokens) and search overlays. Per-bot TOML profiles in [`apa-kan/game/bots/`](apa-kan/game/bots/) still load via `--bot-config` / `--search-profile`.

Headless screenshot (Bevy window under xvfb, or the PNG renderer without `desktop`):

```bash
cargo run -p apa-kan -- --screenshot apa-kan/game/screenshots/board.png --width 1280 --height 800
```

Search head-to-head. Doubles allowed is the locked default. Default search is a work budget (`--budget nodes:132000,iters:2600`) so the same seed replays.

```bash
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- --games 6
```

The locked-ruleset sanity cell (exactplay minimax mirror, light budget, zags on, zag-mismatch sum) lives in [`apa-kan/game/benchmark_results/opening_2211/`](apa-kan/game/benchmark_results/opening_2211/). Historical experiment logs: [`apa-kan/game/benchmark_results/archive/`](apa-kan/game/benchmark_results/archive/).

## Test

```bash
RUSTFLAGS=-Dwarnings cargo check --workspace --all-targets
cargo test --workspace
cargo test -p apa-kan-core
cargo fmt --all -- --check
```

## License

Copyright © 2026 Ramate LLC. All rights reserved.

This software is made available under the Apa Kan Source-Available License. See [LICENSE.md](LICENSE.md) for details.
