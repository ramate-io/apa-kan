# Apa Kan

A turn-based game of colors and adjacency on an 8-petal rosette, built with Rust and Bevy 0.19.1.

## Rules (defaults)

See [rfc/rfc-001-001-001-rosette-redesign](rfc/rfc-001-001-001-rosette-redesign/README.md) for the full spec and every `RuleSet` knob.

- **Board:** 25 nodes (center + three 8-node rings) with circular-arc edges. Eight yellow **spawn** nodes; no two are adjacent.
- **Tiles:** owner is a light (P1) or dark (P2) fill; **green / blue / orange** rings show color; 1–3 rings = weight; 1–3 dots = MOVE length.
- **Bench / bag:** each player is dealt 5 tiles from a shuffled 27-kind bag (identical bags by default). ADD places a bench tile on an empty spawn. Opponent tiles removed on a MOVE draw from the bag onto the bench.
- **Overwhelm (by color):** `matches = weight + same-color neighbors`; `mismatches = different-color neighbors`; removed if `mismatches >= matches`, simultaneously after each action.
- **MOVE:** exactly `d` steps through empty nodes; a hop across a straight occupied run on one circle costs 1 and requires hopped weights `>=` the mover.
- **Win:** opponent has no board tiles and an empty bench. Draw at 200 turns or on a double pass.

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
  --width <px>                               Window width (default: 1280)
  --height <px>                              Window height (default: 900)
  --screenshot <path>                        Capture a mid-game board PNG and exit
  -h, --help
```

Hot-seat: `--bot1 human --bot2 human`. Watch two bots: `--bot1 mcts --bot2 greedy`.

Headless screenshot (Bevy window under xvfb, or the PNG renderer without `desktop`):

```bash
cargo run -p apa-kan -- --screenshot apa-kan/game/screenshots/board.png --width 1280 --height 800
```

Round-robin benchmark:

```bash
cargo run -p apa-kan-bench --release --no-default-features -- --games 20 --think-ms 20
```

Results: [`apa-kan/game/benchmark_results/round_robin.txt`](apa-kan/game/benchmark_results/round_robin.txt).

## Test

```bash
RUSTFLAGS=-Dwarnings cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all -- --check
```

## License

Copyright © 2026 Ramate LLC. All rights reserved.

This software is made available under the Apa Kan Source-Available License. See [LICENSE.md](LICENSE.md) for details.
