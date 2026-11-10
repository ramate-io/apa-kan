# Apa Kan

A turn-based game of colors and adjacency on an 8-petal rosette, built with Rust and Bevy 0.19.1.

## Rules (defaults)

See [rfc/rfc-001-001-001-rosette-redesign](rfc/rfc-001-001-001-rosette-redesign/README.md) for the full spec and every `RuleSet` knob.

- **Board:** 25 nodes (center + three 8-node rings) with circular-arc edges. Eight yellow **spawn** nodes; no two are adjacent.
- **Tiles:** owner is a light (P1) or dark (P2) fill; **green / blue / orange** rings show color; 1–3 rings = weight; 1–3 dots = MOVE length.
- **Bench / bag:** each player is dealt 5 tiles from a shuffled 27-kind bag (identical bags by default). ADD places a bench tile on an empty spawn. Opponent tiles removed on a MOVE draw from the bag onto the bench.
- **Overwhelm (by color):** `matches = weight + same-color neighbors`; `mismatches = different-color neighbors`; removed if `mismatches >= matches`, simultaneously after each action.
- **MOVE:** exactly `d` steps; default path rule is **no edge revisit** (nodes may be revisited via a different arc). A hop across a straight occupied run costs 1; by default hopped weights must be `>=` the mover. Optional `color_restricted_hops` instead allows hopping only lighter tiles of the mover's color.
- **Stall-prevention (defaults):** no legal ADD/MOVE loses immediately; you may not MOVE the same tile on two consecutive turns once your bench is empty (unless the MOVE captures); turn-limit 200 remains a draw. Optional center-hold win (N=5, off).
- **Win:** opponent has no board tiles and an empty bench, or the opponent is locked out with no legal action. Draw at 200 turns. Double-pass draws only if `no_moves_loses` is off.

## Run

```bash
nix develop
cargo run -p apa-kan
```

```text
Usage: apa-kan [OPTIONS]
  --bot1 <human|random|greedy|minimax|mcts>  Player 1, light (default: human)
  --bot2 <human|random|greedy|minimax|mcts>  Player 2, dark (default: greedy)
  --seed <u64>                               Deal/shuffle seed (default: 1)
  --think-ms <n>                             Bot think time in ms (default: 80)
  --delay-ms <n>                             Delay between bot moves (default: 350)
  --actions <1..8>                           Actions per turn (default: 1)
  --turn-limit <n>                           Safety-net draw (default: 200)
  --screenshot <path>                        Write a board PNG and exit
  --no-moves-loses <on|off>                  No ADD/MOVE loses (default: on)
  --center-hold-win <on|off>                 Center-hold win (default: off)
  --center-hold-n <1..64>                    Turns to hold center (default: 5)
  --no-same-piece-twice <off|always|when-bench-empty>
  --color-restricted-hops <on|off>           Hop only own color (default: off)
  --hop-weight-cmp <lt|le>                   Color-hop compare (default: lt)
  --path-constraint <no-node-revisit|no-edge-revisit|both>
  -h, --help
```

Hot-seat: `--bot1 human --bot2 human`. Watch two bots: `--bot1 mcts --bot2 greedy`.

Headless screenshot (no window):

```bash
cargo run -p apa-kan -- --screenshot apa-kan/game/screenshots/board.png
```

Round-robin benchmark:

```bash
cargo run -p apa-kan-bench --release --no-default-features -- --games 10 --think-ms 200 --bots greedy,minimax,mcts
```

Results: [`apa-kan/game/benchmark_results/`](apa-kan/game/benchmark_results/).

## Test

```bash
RUSTFLAGS=-Dwarnings cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all -- --check
```

## License

Copyright © 2026 Ramate LLC. All rights reserved.

This software is made available under the Apa Kan Source-Available License. See [LICENSE.md](LICENSE.md) for details.
