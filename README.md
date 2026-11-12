# Apa Kan

A turn-based game of colors and adjacency on an 8-petal rosette, built with Rust and Bevy 0.19.1.

## Rules (defaults)

See [rfc/rfc-001-001-001-rosette-redesign](rfc/rfc-001-001-001-rosette-redesign/README.md) for the full spec and every `RuleSet` knob.

- **Board:** 25 nodes (center + three 8-node rings) with circular-arc edges. Eight yellow **spawn** nodes; no two are adjacent.
- **Tiles:** owner is a light (P1) or dark (P2) fill. Each tile has 1–3 rings of distinct colors. Color tree: green/blue/orange (81), lavender-pink/teal (9), maroon (1). Ring 1 always exists; rings 2–3 keep the last color with weight 7 or draw any color with weight 1. Each ring is independently a **zag** with chance 1/7 (`--zag-chance 0` / `--zags off` disables). A zag never matches. A zag on either tile whose color the other holds is a mismatch on the **edge**; both tiles take that count (`--zag-mismatch sum` default adds every hitting zag stroke; `max` takes, per color, the larger of the two tiles' hitting counts of that color). Weight 1–3 is the overwhelm base (and hop compare when `--graph-hops` is on); dots 1–3 are MOVE length. Concentric strokes show ring colors (zigzag stroke = zag); extra weight strokes when weight > ring count.
- **Bench / bag / budget:** each player is dealt 7 tiles from a shuffled bag of 27 independently sampled tiles (identical bags by default; `--bag-size N` is the total deck, default 27 so a deal of 7 leaves 20 to draw; `--starting-bench N` deals N and starts the capture-budget stack at N blank tokens). Any remaining bench tile may be chosen at any time; ADD is gated by **usable budget** = min(tokens in the stack, tiles on the bench), not by which tile you pick. Tokens beyond the bench stay on the stack (the UI fades that surplus) but cannot be spent until a later bag draw puts a tile on the bench. ADD spends 1 token. Every opponent capture earns 1 (the captured tile is pushed onto the budget stack). Capturing your own tile on your turn also earns 1 by default (`--self-capture-budget on`); `--self-capture-budget off` still removes the tile but does not refund a token. Opponent tiles removed on a MOVE still draw from the bag onto the bench. The budget stack sits beside the bench: opening blanks plus captured tiles as they happen; spending removes one from the front (blanks first).
- **Overwhelm (by color):** a neighbor **matches** if the two tiles share a **normal** ring color. Zags never produce a match. Zag mismatches are undirected: `sum` (default) adds every hitting zag stroke on both tiles; `max` still counts up but per color takes the larger of the two tiles' hitting zag-stroke counts of that color. Both tiles take that edge count. If that is 0 and you share no normal color, the ordinary +1 mismatch. `matches = weight + matching neighbors`; removed if `mismatches >= matches`, simultaneously after each action. Non-matching zags are empty. A tile can still mismatch on its normal rings even if it also has zags.
- **MOVE:** exactly `d` steps; default path rule is **no edge revisit** (nodes may be revisited via a different arc). Default movement is **counted pass-through**: each adjacent step costs 1; a tile may pass through (not land on) an occupied node that shares a **normal** color and whose ring count compares with `--pass-through-ring-cmp` (default `ge`: occupant `n_rings >=` mover; `gt` is strict, so 3-ring tiles never pass through). Each pass-through node costs one of the exact-`d` steps. Zag colors do not enable pass-through. Optional `--graph-hops on` restores graph hops: a hop follows any adjacency through a connected chain of hoppable occupied tiles (shared normal, weight `<=` mover under default `le`) and may change direction at any node, center included. Landing is any empty node adjacent to that chain; the hop still costs 1. With color hops off, hopped weights must be `>=` the mover (any color).
- **Stall-prevention (defaults):** no legal ADD/MOVE loses immediately; you may not MOVE the same tile on two consecutive turns once your bench is empty (unless the MOVE captures); occupy 5 of 9 (center + inner ring) for 1 own turn to win; if neither player can ADD (both **usable** budgets 0, including leftover tokens on an empty bench) and both have fewer than 5 board tiles, the player holding the lowest-value piece wins (value = dots + rings; on a tie, recompute with zag rings worth 2; still tied is a draw — no next-piece fallthrough). Turn-limit 200 remains a draw. Optional `--center-hold-win` wins by occupying the center for 5 consecutive own turns. Optional `--petal-majority-win` wins by occupying 5 of 8 inner-ring (petal) nodes. Optional `--insufficient-tiles-loss` is a can't-reach-control loss.
- **Win:** opponent has no board tiles and an empty bench, the opponent is locked out with no legal action, you occupy 5 of the 9-node control set (center + inner ring) for 1 own turn, or the low-value condition fires after lockout. Draw at 200 turns, or if low-value keys fully match. Double-pass draws only if `no_moves_loses` is off.

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
  --scenario <name|path>                     Opening deck (default|same|complementary|...)
  --list-scenarios                           Print builtin deck scenarios
  --no-moves-loses <on|off>                  No ADD/MOVE loses (default: on)
  --center-hold-win <on|off>                 Center-hold win (default: off)
  --center-hold-n <1..64>                    Turns to hold center (default: 5)
  --no-same-piece-twice <off|always|when-bench-empty>
  --color-restricted-hops <on|off>           Hop only own color (default: on; hops off by default)
  --hop-weight-cmp <lt|le>                   Color-hop compare (default: le)
  --counted-pass-through <on|off>            Pass through occupied tiles at 1/step (default: on)
  --graph-hops <on|off>                      Restore graph hops (alias: counted-pass-through off)
  --pass-through-ring-cmp <ge|gt>            Pass-through ring gate (default: ge)
  --petal-majority-win <on|off>              Inner-ring majority win (default: off)
  --petal-majority-n <1..64>                 Turns to hold majority (default: 1)
  --control-win <on|off>                     Center+inner-ring control win (default: on)
  --control-n <1..9>                         Control-set occupancy needed (default: 5)
  --control-hold-turns <1..64>               Turns to hold control (default: 1)
  --insufficient-tiles-loss <off|strict|no-recapture>
                                             Can't-reach-control loss (default: off)
  --starting-bench <1..27>                   Opening deal and capture budget (default: 7)
  --bag-size <1..27>                         Total sampled tiles per player (default: 27)
  --self-capture-budget <on|off>             Own-tile capture earns budget (default: on)
  --max-rings <1..3>                         Distinct rings per tile (default: 3)
  --zag-chance <0..7>                        Each ring is a zag with k/7 chance (default: 1)
  --zags <on|off>                            Alias for --zag-chance 1 or 0 (default: on)
  --zag-mismatch <sum|max>                   Zag edge count (default: sum)
  --path-constraint <no-node-revisit|no-edge-revisit|both>
  -h, --help
```

A plain `cargo run -p apa-kan` uses the new defaults (control-win 5 of 9 at t=1, 7-tile bench, center-hold off). Restore the previous center-hold / 5-tile package with `--center-hold-win on --control-win off --starting-bench 5`.

Hot-seat: `--bot1 human --bot2 human`. Watch two bots: `--bot1 mcts --bot2 greedy`.

Deck scenarios (see [`apa-kan/game/scenarios/`](apa-kan/game/scenarios/)): `--scenario same` is the default mirrored generative 27-tile deal; `--scenario independent` samples each player's bag separately; `--scenario complementary` gives P1 green+orange and P2 blue+orange (18 one-ring tiles each, same weight/dot order); `--scenario complementary-swapped` flips the exclusive colors. Tiles may list `colors = ["green", "teal"]` for multiple normal rings.

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
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --scenario complementary --games 12 --think-ms 1000 --no-depth-probe
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --zags off --games 12 --think-ms 1000 --no-depth-probe
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --graph-hops on --games 12 --think-ms 1000 --no-depth-probe
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --pass-through-ring-cmp gt --games 12 --think-ms 1000 --no-depth-probe
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --zag-mismatch max --games 12 --think-ms 1000 --no-depth-probe
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --bag-size 11 --games 12 --think-ms 1000 --no-depth-probe
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --bag-size 11 --self-capture-budget off --games 12 --think-ms 1000 --no-depth-probe
```

Results: [`apa-kan/game/benchmark_results/search_h2h.txt`](apa-kan/game/benchmark_results/search_h2h.txt). One full minimax vs MCTS move list: [`apa-kan/game/benchmark_results/minimax_vs_mcts_game.txt`](apa-kan/game/benchmark_results/minimax_vs_mcts_game.txt). Stall-prevention logs: [`apa-kan/game/benchmark_results/`](apa-kan/game/benchmark_results/). Defaults + center-hold (N=5) + color-restricted hops at 1000 ms/move, `lt` vs `le`: [`defaults_center_hold_color_hops_lt.txt`](apa-kan/game/benchmark_results/defaults_center_hold_color_hops_lt.txt), [`defaults_center_hold_color_hops_le.txt`](apa-kan/game/benchmark_results/defaults_center_hold_color_hops_le.txt). After scoring hold terminals from the mover: [`defaults_center_hold_color_hops.txt`](apa-kan/game/benchmark_results/defaults_center_hold_color_hops.txt), plus petal-majority N=1 [`defaults_plus_petal_majority_n1.txt`](apa-kan/game/benchmark_results/defaults_plus_petal_majority_n1.txt), petal-majority N=3 instead of center-hold [`petal_majority_n3_no_center_hold.txt`](apa-kan/game/benchmark_results/petal_majority_n3_no_center_hold.txt), and new vs legacy [`minimax_new_vs_legacy.txt`](apa-kan/game/benchmark_results/minimax_new_vs_legacy.txt). Control-win (center-hold off) at t=1 [`control_win_t1.txt`](apa-kan/game/benchmark_results/control_win_t1.txt), t=3 [`control_win_t3.txt`](apa-kan/game/benchmark_results/control_win_t3.txt), t=1 + insufficient-tiles strict [`control_win_t1_strict.txt`](apa-kan/game/benchmark_results/control_win_t1_strict.txt), t=1 + no-recapture [`control_win_t1_no_recapture.txt`](apa-kan/game/benchmark_results/control_win_t1_no_recapture.txt). Same three with `--starting-bench 7`: [`control_win_t1_bench7.txt`](apa-kan/game/benchmark_results/control_win_t1_bench7.txt) (now the default package: control-win t=1, center-hold off, bench 7, insufficient-tiles off), [`control_win_t1_bench7_strict.txt`](apa-kan/game/benchmark_results/control_win_t1_bench7_strict.txt), [`control_win_t1_bench7_no_recapture.txt`](apa-kan/game/benchmark_results/control_win_t1_bench7_no_recapture.txt). Deck fairness at 12 games/pairing/seat on those defaults: [`deck_fairness_default.txt`](apa-kan/game/benchmark_results/deck_fairness_default.txt) (same/mirrored bags), [`deck_fairness_independent.txt`](apa-kan/game/benchmark_results/deck_fairness_independent.txt), [`deck_fairness_complementary.txt`](apa-kan/game/benchmark_results/deck_fairness_complementary.txt), [`deck_fairness_complementary_swapped.txt`](apa-kan/game/benchmark_results/deck_fairness_complementary_swapped.txt). Six-color multi-ring default vs `--max-rings 1` (release, 12/pairing/seat, 1000 ms): [`multi_ring_default.txt`](apa-kan/game/benchmark_results/multi_ring_default.txt), [`multi_ring_single.txt`](apa-kan/game/benchmark_results/multi_ring_single.txt). Zags on (1/7) vs zags off under graph hops: [`zag_rings_default.txt`](apa-kan/game/benchmark_results/zag_rings_default.txt), [`zag_rings_off.txt`](apa-kan/game/benchmark_results/zag_rings_off.txt). Counted pass-through (now the default) GE vs GT, zags 1/7 and off: [`pass_through_ge_default.txt`](apa-kan/game/benchmark_results/pass_through_ge_default.txt), [`pass_through_ge_zags_off.txt`](apa-kan/game/benchmark_results/pass_through_ge_zags_off.txt), [`pass_through_gt_default.txt`](apa-kan/game/benchmark_results/pass_through_gt_default.txt), [`pass_through_gt_zags_off.txt`](apa-kan/game/benchmark_results/pass_through_gt_zags_off.txt). Undirected zag-mismatch SUM vs MAX (pass-through GE, zags 1/7): [`zag_mismatch_sum.txt`](apa-kan/game/benchmark_results/zag_mismatch_sum.txt), [`zag_mismatch_max.txt`](apa-kan/game/benchmark_results/zag_mismatch_max.txt). Capture budget + low-value win on that SUM default: [`capture_budget_low_value.txt`](apa-kan/game/benchmark_results/capture_budget_low_value.txt). Smaller generative bags (7 dealt) × self-capture budget on/off: [`deck11_self_capture_on.txt`](apa-kan/game/benchmark_results/deck11_self_capture_on.txt), [`deck11_self_capture_off.txt`](apa-kan/game/benchmark_results/deck11_self_capture_off.txt), [`deck13_self_capture_on.txt`](apa-kan/game/benchmark_results/deck13_self_capture_on.txt), [`deck13_self_capture_off.txt`](apa-kan/game/benchmark_results/deck13_self_capture_off.txt). Earlier optional-pass-through logs: [`counted_pass_through_default.txt`](apa-kan/game/benchmark_results/counted_pass_through_default.txt), [`counted_pass_through_off.txt`](apa-kan/game/benchmark_results/counted_pass_through_off.txt). The earlier 20 ms round-robin is in [`apa-kan/game/benchmark_results/round_robin.txt`](apa-kan/game/benchmark_results/round_robin.txt).

## Test

```bash
RUSTFLAGS=-Dwarnings cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all -- --check
```

## License

Copyright © 2026 Ramate LLC. All rights reserved.

This software is made available under the Apa Kan Source-Available License. See [LICENSE.md](LICENSE.md) for details.
