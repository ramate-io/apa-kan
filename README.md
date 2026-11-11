# Apa Kan

A turn-based game of colors and adjacency on an 8-petal rosette, built with Rust and Bevy 0.19.1.

## Rules (defaults)

See [rfc/rfc-001-001-001-rosette-redesign](rfc/rfc-001-001-001-rosette-redesign/README.md) for the full spec and every `RuleSet` knob.

- **Board:** 25 nodes (center + three 8-node rings) with circular-arc edges. Eight yellow **spawn** nodes; no two are adjacent.
- **Tiles:** owner is a light (P1) or dark (P2) fill; **green / blue / orange** rings show color; 1–3 rings = weight; 1–3 dots = MOVE length.
- **Bench / bag:** each player is dealt 5 tiles from a shuffled 27-kind bag (identical bags by default; `--starting-bench N` deals N, still from 27). ADD places a bench tile on an empty spawn. Opponent tiles removed on a MOVE draw from the bag onto the bench.
- **Overwhelm (by color):** `matches = weight + same-color neighbors`; `mismatches = different-color neighbors`; removed if `mismatches >= matches`, simultaneously after each action.
- **MOVE:** exactly `d` steps; default path rule is **no edge revisit** (nodes may be revisited via a different arc). A hop across a straight occupied run costs 1. Default `color_restricted_hops` allows hopping only same-color tiles whose weight is `<=` the mover (`le`; `lt` is stricter). With color hops off, hopped weights must be `>=` the mover (any color).
- **Stall-prevention (defaults):** no legal ADD/MOVE loses immediately; you may not MOVE the same tile on two consecutive turns once your bench is empty (unless the MOVE captures); occupy the center for 5 consecutive own turns to win; turn-limit 200 remains a draw. Optional `--petal-majority-win` wins by occupying 5 of 8 inner-ring (petal) nodes. Optional `--control-win` wins by occupying 5 of the 9-node control set (center + inner ring). Optional `--insufficient-tiles-loss` is a can't-reach-control loss.
- **Win:** opponent has no board tiles and an empty bench, the opponent is locked out with no legal action, or you hold the center for N own turns (default 5). Draw at 200 turns. Double-pass draws only if `no_moves_loses` is off.

## Run

```bash
nix develop
cargo run -p apa-kan
```

```text
Usage: apa-kan [OPTIONS]
  --bot1 <human|random|greedy|minimax|minimax-legacy|mcts>  Player 1, light (default: human)
  --bot2 <human|random|greedy|minimax|minimax-legacy|mcts>  Player 2, dark (default: greedy)
  --seed <u64>                               Deal/shuffle seed (default: 1)
  --think-ms <n>                             Bot think time in ms (default: 80)
  --delay-ms <n>                             Delay between bot moves (default: 350)
  --actions <1..8>                           Actions per turn (default: 1)
  --turn-limit <n>                           Safety-net draw (default: 200)
  --width <px>                               Window width (default: 1280)
  --height <px>                              Window height (default: 900)
  --screenshot <path>                        Capture a mid-game board PNG and exit
  --no-moves-loses <on|off>                  No ADD/MOVE loses (default: on)
  --center-hold-win <on|off>                 Center-hold win (default: on)
  --center-hold-n <1..64>                    Turns to hold center (default: 5)
  --no-same-piece-twice <off|always|when-bench-empty>
  --color-restricted-hops <on|off>           Hop only own color (default: on)
  --hop-weight-cmp <lt|le>                   Color-hop compare (default: le)
  --petal-majority-win <on|off>              Inner-ring majority win (default: off)
  --petal-majority-n <1..64>                 Turns to hold majority (default: 1)
  --control-win <on|off>                     Center+inner-ring control win (default: off)
  --control-n <1..9>                         Control-set occupancy needed (default: 5)
  --control-hold-turns <1..64>               Turns to hold control (default: 1)
  --insufficient-tiles-loss <off|strict|no-recapture>
                                             Can't-reach-control loss (default: off)
  --starting-bench <1..27>                   Opening deal from the 27-kind bag (default: 5)
  --path-constraint <no-node-revisit|no-edge-revisit|both>
  -h, --help
```

Hot-seat: `--bot1 human --bot2 human`. Watch two bots: `--bot1 mcts --bot2 greedy`.

Watch minimax vs MCTS in the GUI (1 s/move):

```bash
cargo run -p apa-kan --release -- --bot1 minimax --bot2 mcts --think-ms 1000 --delay-ms 250
cargo run -p apa-kan --release -- --bot1 mcts --bot2 minimax --think-ms 1000 --delay-ms 250
```

Headless screenshot (Bevy window under xvfb, or the PNG renderer without `desktop`):

```bash
cargo run -p apa-kan -- --screenshot apa-kan/game/screenshots/board.png --width 1280 --height 800
```

Search head-to-head (branching factor, stalling, minimax/MCTS/greedy):

```bash
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- --games 6 --think-ms 1000
```

Results: [`apa-kan/game/benchmark_results/search_h2h.txt`](apa-kan/game/benchmark_results/search_h2h.txt). One full minimax vs MCTS move list: [`apa-kan/game/benchmark_results/minimax_vs_mcts_game.txt`](apa-kan/game/benchmark_results/minimax_vs_mcts_game.txt). Stall-prevention logs: [`apa-kan/game/benchmark_results/`](apa-kan/game/benchmark_results/). Defaults + center-hold (N=5) + color-restricted hops at 1000 ms/move, `lt` vs `le`: [`defaults_center_hold_color_hops_lt.txt`](apa-kan/game/benchmark_results/defaults_center_hold_color_hops_lt.txt), [`defaults_center_hold_color_hops_le.txt`](apa-kan/game/benchmark_results/defaults_center_hold_color_hops_le.txt). After scoring hold terminals from the mover: [`defaults_center_hold_color_hops.txt`](apa-kan/game/benchmark_results/defaults_center_hold_color_hops.txt), plus petal-majority N=1 [`defaults_plus_petal_majority_n1.txt`](apa-kan/game/benchmark_results/defaults_plus_petal_majority_n1.txt), petal-majority N=3 instead of center-hold [`petal_majority_n3_no_center_hold.txt`](apa-kan/game/benchmark_results/petal_majority_n3_no_center_hold.txt), and new vs legacy [`minimax_new_vs_legacy.txt`](apa-kan/game/benchmark_results/minimax_new_vs_legacy.txt). Control-win (center-hold off) at t=1 [`control_win_t1.txt`](apa-kan/game/benchmark_results/control_win_t1.txt), t=3 [`control_win_t3.txt`](apa-kan/game/benchmark_results/control_win_t3.txt), t=1 + insufficient-tiles strict [`control_win_t1_strict.txt`](apa-kan/game/benchmark_results/control_win_t1_strict.txt), t=1 + no-recapture [`control_win_t1_no_recapture.txt`](apa-kan/game/benchmark_results/control_win_t1_no_recapture.txt). The earlier 20 ms round-robin is in [`apa-kan/game/benchmark_results/round_robin.txt`](apa-kan/game/benchmark_results/round_robin.txt).

## Test

```bash
RUSTFLAGS=-Dwarnings cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all -- --check
```

## License

Copyright © 2026 Ramate LLC. All rights reserved.

This software is made available under the Apa Kan Source-Available License. See [LICENSE.md](LICENSE.md) for details.
