# Apa Kan

A turn-based game of colors and adjacency on an 8-petal rosette, built with Rust and Bevy 0.19.1.

## Rules (defaults)

See [rfc/rfc-001-001-001-rosette-redesign](rfc/rfc-001-001-001-rosette-redesign/README.md) for the full spec and every `RuleSet` knob.

- **Board:** 25 nodes (center + three 8-node rings) with circular-arc edges. Eight yellow **spawn** nodes; no two are adjacent.
- **Tiles:** owner is a light (P1) or dark (P2) fill. Each tile has 1–3 rings of distinct colors. Color tree: green/blue/orange (81), lavender-pink/teal (9), maroon (1). Ring count is drawn from `--ring-count-weights` (default **4:3:1**). Each ring slot rolls a heavy coin (default **5:3** light:heavy); **two or more consecutive heavies** become a braid. A lone heavy on a **1-ring** tile becomes a 1-slot braid (`--single-braids on`, same coin, no extra roll); a lone or gapped heavy on a 2–3 ring tile falls through to the light election. `--single-braids off` restores fall-through for 1-rings too. Remaining light slots elect **normal : zag : bead = 5 : 3 : 1**. `--zags off` / `--bead-chance 0` drop that type. `--channel-mode` only changes scoring, not generation. A zag never matches. Beads are a **separate Additive channel** (unmatched bead rings +1 each; a matched bead is −1 at most once). Default scoring is **additive** (every firing channel contributes; `--bead-scoring independent` = beads separate). `--channel-mode independent-max` restores max of any one channel ([`rfc-001-001-008`](rfc/rfc-001-001-008-independent-max-channels/README.md)) and folds bead discs onto the normal HUD lane. `--bead-scoring shared` restores `max(normal, bead)` folding on Additive edges. `--braid-scoring braid-only-exact` (default): only an **exact** braid match scores (1-slot +1); other matching channels still **paint** as matches and occupy, but score 0. `--braid-scoring current` restores coverage scoring. `--color-drift 160` (default) makes multicolor rarer; `--color-drift 0` restores uniqueness. `--same-deck on` (default identical bags) deals both players the same 27-tile collection. Weight is the live ring-slot count (braid slots included). Dots 1–3 are MOVE length, drawn as that many **capture marks**: filled circles (**dot**: captures join the mover's bench) or filled triangles (**triangle**: captures become spawn tokens). Every tile has a mark (`--dot-chance`, default 50/50). Each tile also has one optional **emblem** at the top (**None** 200 / **Crescent Moon** 1 / **Two Zag Zs** 1 / **Blocker** 1 / **Rosette** 1; `--emblem-weights`, `--emblems on|off`). A non-empty emblem except Rosette adds +1 to deck cost, not to overwhelm weight. Crescent Moon: mismatches equal to the tile's rings do not overwhelm it when it has exactly one occupied neighbor. Two Zag Zs: +1 on the zag channel per incident edge that already has a zag mismatch. Concentric strokes show one ring per slot (zigzag stroke = zag; dotted discs = bead, drawn on the normal lane). An emblem opens a top C-notch in those rings.
- **Deck / spawn tokens:** two separate concepts. Each player has a **27-tile collection** (`--deck-size` / `--bag-size N`, default 27) and drafts an **11-tile public bench**: secret 7, both reveal, then 4 from reserves with the opponent's 7 in view. Leftover **reserves** are never usable in play. The full 11-tile bench must have at most **6 rings of each non-normal type** (zag, braid, bead). Counted by **ring slots** (confirmed): a 3-braid uses 3 of the 6. Normal is uncapped. Enforced at the 7-pick and the +4. `--pick heuristic|search|greedy` (default heuristic) chooses the 7+4; `--deck-seed` (default `--seed`) and `--deck-noise 0..100` control generation. Spawn tokens start at 7 (`--starting-tokens` / `--starting-bench N`) and are independent of collection size. ADD is gated by **usable budget** = min(tokens, unplaced **bench** tiles). Tokens beyond the remaining bench stay on the stack as opaque blank faces. ADD spends 1 token. Captures from a **triangle** tile earn 1 token each (the captured tile is pushed onto the token stack; own-tile captures still gated by `--self-capture-budget`). Captures from a **dot** tile join the mover's bench as unplaced tiles they now own (including self-captures) and may push the bench past 11 tiles and the 6-per-type cap — that cap applies only at 7+4 selection. Captures do not draw extra tiles. The token stack sits on the left of the deck as solid filled discs (opening blanks are opaque owner-fill faces; captured tokens are solid too). Spending removes one from the front (blanks first). The deck row shows only unplaced **bench** tiles (all 11 at game start); a tile added to the board leaves the row.
- **Overwhelm (by color):** each channel (normal, zag, bead, braid) scores on its own. Default `--channel-mode additive`: channels stack (beads separate). `--channel-mode independent-max`: the edge's match and mismatch are the **max of any single channel**, then `effective` sums those signed edges. `matches = ring count + matching neighbors`; a tile is overwhelmed when `effective >= weight`. Center↔inner links are **double-counted** (per generating-circle petal). `--link-once on` restores unique-neighbor scoring. Default `--capture-mode local-claim`: ADD/MOVE remove only the placed/moved tile (if self-overwhelmed) and its immediate destination neighbors. Other tiles that become overwhelmed stay on the board until healed or **CLAIM**ed. `--capture-mode cascade` restores PR #70's full-board simultaneous sweep. Beads do not enable pass-through.
- **CLAIM:** instead of ADD or MOVE, click (or search) any one currently overwhelmed tile — own or opponent. The tile leaves play like a capture and **always** awards the claiming player a spawn token (no token cost; ignores the tile's dot/triangle mark and `--self-capture-budget`). Claim does not cascade. Claim counts as a legal action for lockout.
- **SWAP:** a 1-ring **normal** may swap with any **adjacent** occupied tile (own or opponent) as a full action (`--singles-swap normal-only`). No spawn token. Same unordered pair cannot swap back on the immediately following move. Cannot target a vacant node (including an empty center). `--singles-swap off` disables SWAP; `--singles-swap on` lets any 1-ring (braid/bead/zag included) initiate. The HUD paints pink rings only on legal SWAP partners.
- **MOVE:** exactly `d` steps; default path rule is **no edge revisit** (nodes may be revisited via a different arc). Default movement is **counted pass-through**: each adjacent step costs 1; a tile may pass through (not land on) an occupied node that shares a **normal** color and whose ring count compares with `--pass-through-ring-cmp` (default `ge`: occupant `n_rings >=` mover; `gt` is strict, so 3-ring tiles never pass through). Each pass-through node costs one of the exact-`d` steps. Zag colors do not enable pass-through. Optional `--graph-hops on` restores graph hops: a hop follows any adjacency through a connected chain of hoppable occupied tiles (shared normal, ring count `<=` mover under default `le`) and may change direction at any node, center included. Landing is any empty node adjacent to that chain; the hop still costs 1. With color hops off, hopped ring counts must be `>=` the mover (any color).
- **Stall-prevention (defaults):** no legal ADD/MOVE/SWAP/CLAIM loses immediately; repeat MOVEs of the same tile are allowed (`--no-same-piece-twice off`; `always` / `when-deck-empty` remain as variants). Default `--endgame-rule control-cascade`: a one-way ratchet starts at 5 of the 9-node control set (center + inner ring) and lowers via `target = target.min(max_mat)` while `max_mat = max(p1, p2)` of `board + usable budget` is at least 2 (checkpoints 5→4→3→2; recaptures never raise it). Occupying `>= target` for 1 own turn wins. If an action drops the ratchet onto a checkpoint held by **exactly one** player, that player wins immediately (no hold). If both already hold the new checkpoint, play continues under the turn-boundary hold. When `max_mat ≤ 1`, control is inactive and the existing low-value pair check is the tie-breaker (same piece-value order: dots + rings, then zag rings worth 2; a missing piece counts as max value). `--endgame-rule low-value` restores the baseline: fixed 5 of 9, then low-value when both sides are strictly below 5. Usable budget is min(tokens, unplaced deck tiles); surplus tokens on an empty remaining deck do not count. Turn-limit 200 remains a draw. Optional `--center-hold-win` wins by occupying the center for 5 consecutive own turns. Optional `--petal-majority-win` wins by occupying 5 of 8 inner-ring (petal) nodes. Optional `--insufficient-tiles-loss` is a can't-reach-control loss (still keyed off the static `control_n` cap, not the cascade ratchet).
- **Win:** opponent has no board tiles and an empty remaining deck, the opponent is locked out with no legal ADD/MOVE/SWAP/CLAIM, you hold the ratcheted control target of the 9-node set (immediate unique-holder drop, or 1 own turn), or low-value fires (`max_mat ≤ 1` under cascade; both below 5 under `--endgame-rule low-value`). Draw at 200 turns, or if low-value pair keys match at equal piece counts. Double-pass draws only if `no_moves_loses` is off.

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
  --deck-seed <u64>                          Collection generation seed (default: --seed)
  --deck-noise <0..100>                      Resample that percent of a mirrored collection (default: 0)
  --pick <heuristic|search|greedy>           Bench 7+4 picker (default: heuristic)
  --think-ms <n>                             Bot think time in ms (default: 80)
  --delay-ms <n>                             Delay between bot moves (default: 350)
  --actions <1..8>                           Actions per turn (default: 1)
  --turn-limit <n>                           Safety-net draw (default: 200)
  --width <px>                               Window width (default: 1280)
  --height <px>                              Window height (default: 900)
  --screenshot <path>                        Capture a mid-game board PNG and exit
  --scenario <name|path>                     Opening deck (default|same|complementary|...)
  --list-scenarios                           Print builtin deck scenarios
  --no-moves-loses <on|off>                  No ADD/MOVE/CLAIM loses (default: on)
  --center-hold-win <on|off>                 Center-hold win (default: off)
  --center-hold-n <1..64>                    Turns to hold center (default: 5)
  --no-same-piece-twice <off|always|when-deck-empty>
                                              Repeat-piece rule (default: off)
  --color-restricted-hops <on|off>           Hop only own color (default: on; hops off by default)
  --hop-weight-cmp <lt|le>                   Color-hop compare (default: le)
  --counted-pass-through <on|off>            Pass through occupied tiles at 1/step (default: on)
  --graph-hops <on|off>                      Restore graph hops (alias: counted-pass-through off)
  --pass-through-ring-cmp <ge|gt>            Pass-through ring gate (default: ge)
  --petal-majority-win <on|off>              Inner-ring majority win (default: off)
  --petal-majority-n <1..64>                 Turns to hold majority (default: 1)
  --control-win <on|off>                     Center+inner-ring control win (default: on)
  --control-n <1..9>                         Control-set occupancy cap (default: 5)
  --control-hold-turns <1..64>               Turns to hold control (default: 1)
  --endgame-rule <control-cascade|low-value> One-way ratchet 5→2 with max material, then low-value at ≤1 (default: control-cascade)
  --insufficient-tiles-loss <off|strict|no-recapture>
                                             Can't-reach-control loss (default: off)
  --starting-tokens <1..27>                  Opening spawn tokens (default: 7; alias --starting-bench)
  --deck-size <1..27>                        Collection size per player (default: 27; alias --bag-size)
  --self-capture-budget <on|off>             Own-tile capture earns budget (default: on)
  --max-rings <1..3>                         Distinct rings per tile (default: 3)
  --zag-chance <0..7>                        Each ring is a zag with k/7 chance (default: 1)
  --zags <on|off>                            Alias for --zag-chance 1 or 0 (default: on)
  --braid-chance <0..7>                      A piece forms a braid with k/7 chance (default: 1)
  --braids <on|off>                          Alias for --braid-chance 1 or 0 (default: on)
  --bead-chance <0..7>                       Bead light-election weight (default: 1; 0 disables)
  --dot-chance <0..100>                      Generated tiles that are dots (rest triangles; default: 50)
	   --emblem-weights <none,crescent,zs>        Emblem generation weights (default: 200,1,1)
	   --emblems <on|off>                         Alias for default weights or 1,0,0 (skip RNG)
	   --p1-deck <mixed|braid|bead|crescent|zs|emblem>  P1 collection archetype
	   --p2-deck <mixed|braid|bead|crescent|zs|emblem>  P2 collection archetype
	   --zag-mismatch <sum|max>                   Zag edge count (default: sum)
  --channel-mode <independent-max|additive>  Edge combine only (default: additive)
  --color-drift <n>                          RFC ColorPass.drift diagonal (default 160)
  --same-deck <on|off>                       Identical 27-tile start for both players (default: on)
  --capture-mode <local-claim|cascade>       Local ADD/MOVE/SWAP captures + Claim (default: local-claim)
  --singles-swap <on|off|normal-only>        Adjacent singles-swap (default: normal-only; on = any 1-ring; alias --swap-mode)
  --braid-scoring <current|braid-only-match|braid-only-exact>
                                             Braid match total (default: braid-only-exact)
  --single-braids <on|off>                   Lone heavies become 1-slot braids (default: on)
  --link-once <on|off>                       Count each unique node pair once (default: off; live scoring is per-arc)
  --ring-count-weights <s,d,t>               Singles:doubles:triples (default: 4,3,1)
  --braid-fail-mismatch <one|weight|sum>     Failed-braid mismatch (default: one)
  --path-constraint <no-node-revisit|no-edge-revisit|both>
  -h, --help
```

A plain `cargo run -p apa-kan` is the playtest package: **4:3:1** rings, **Additive** beads-separate, **braid-only-exact** (1-slot braids +1 on exact match; other matches paint but score 0), **single braids on**, center↔inner **double-count**, **normal-only SWAP**, **color-drift 160**, local-claim, control-cascade, 7 spawn tokens, 27-tile collection drafted to an 11-tile public bench, doubles allowed. Restore independent-max with `--channel-mode independent-max`. Restore PR #70 cascading captures with `--capture-mode cascade`. Restore the fixed-5 + both-below-5 endgame with `--endgame-rule low-value`. Restore the previous center-hold / 5-token package with `--center-hold-win on --control-win off --starting-tokens 5`. Restore the late-game double-move restriction with `--no-same-piece-twice when-deck-empty`. Restore no-swap with `--singles-swap off`. Restore any-1-ring swap with `--singles-swap on`. Restore coverage braid scoring with `--braid-scoring current`. Restore uniqueness with `--color-drift 0`. Restore 7:5:4 rings with `--ring-count-weights 7,5,4`. `--link-once on` is the rejected unique-neighbor experiment (kept as a restore).

Playtest (human vs greedy, live defaults, no extra flags):

```bash
cargo run --no-default-features -p apa-kan
```

With a desktop window: `nix develop && cargo run -p apa-kan`. Hot-seat: `--bot1 human --bot2 human`. Watch two bots: `--bot1 mcts --bot2 greedy`. Live search is `--search-profile enhanced`. `--search-profile exactplay` is the same tree with `eval_margin_scale=200` (hillclimbed vs enhanced; [`exactplay`](apa-kan/game/benchmark_results/exactplay/)). Setup: pick 7, both reveal, then 4 from reserves (cap 6 rings of each non-normal type; a 1-slot braid uses 1 of 6). In play: pink rings are SWAP partners and only appear when you select a **1-ring normal**. A 1-slot braid is a single weave ring. Non-braid matches still paint as matches; they score 0.

Deck scenarios (see [`apa-kan/game/scenarios/`](apa-kan/game/scenarios/)): `--scenario same` is the default mirrored generative 27-tile collection; `--scenario independent` samples each player's collection separately; `--scenario complementary` gives P1 green+orange and P2 blue+orange (18 one-ring tiles each, same dots order; weight is ring count; 11-tile bench + 7 reserves); `--scenario complementary-swapped` flips the exclusive colors. Tiles may list `colors = ["green", "teal"]` for multiple normal rings.

Watch minimax vs MCTS in the GUI (1 s/move):

```bash
cargo run -p apa-kan --release -- --bot1 minimax --bot2 mcts --think-ms 1000 --delay-ms 250
cargo run -p apa-kan --release -- --bot1 mcts --bot2 minimax --think-ms 1000 --delay-ms 250
```

Headless screenshot (Bevy window under xvfb, or the PNG renderer without `desktop`):

```bash
cargo run -p apa-kan -- --screenshot apa-kan/game/screenshots/board.png --width 1280 --height 800
```

Search head-to-head (branching factor, stalling, minimax/MCTS/greedy). Doubles allowed (`--no-same-piece-twice off`) is the locked default and the **only benchmarked** configuration. Default search is a work budget (`--budget nodes:132000,iters:2600`, calibrated to 1000 ms single-threaded) so the same seed replays. `--jobs` runs independent games in parallel (default: available cores); `--think-ms` is the older wall-clock bound.

```bash
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- --games 6
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --games 12 --seed 1 --no-depth-probe
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --games 12 --think-ms 1000 --seed 1 --no-depth-probe
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
  --bag-size 27 --games 12 --think-ms 1000 --no-depth-probe
cargo run -p apa-kan --bin apa-kan-bench --release --no-default-features -- \
  --self-capture-budget off --games 12 --think-ms 1000 --no-depth-probe
```

Control-cascade shallow H2H (local-claim, independent-max): [`control_cascade`](apa-kan/game/benchmark_results/control_cascade/). Light independent-max vs additive H2H (doubles allowed, zags on): [`independent_max`](apa-kan/game/benchmark_results/independent_max/). Current 108-game H2H (doubles allowed, work budget, zag-color exclusion): [`zag_color_exclusion`](apa-kan/game/benchmark_results/zag_color_exclusion/). Previous capture-modifiers package: [`capture_modifiers`](apa-kan/game/benchmark_results/capture_modifiers/). PR #40 baseline: [`work_budget_doubles_allowed`](apa-kan/game/benchmark_results/work_budget_doubles_allowed/). Older logs: [`apa-kan/game/benchmark_results/search_h2h.txt`](apa-kan/game/benchmark_results/search_h2h.txt). One full minimax vs MCTS move list: [`apa-kan/game/benchmark_results/minimax_vs_mcts_game.txt`](apa-kan/game/benchmark_results/minimax_vs_mcts_game.txt). Stall-prevention logs: [`apa-kan/game/benchmark_results/`](apa-kan/game/benchmark_results/). Defaults + center-hold (N=5) + color-restricted hops at 1000 ms/move, `lt` vs `le`: [`defaults_center_hold_color_hops_lt.txt`](apa-kan/game/benchmark_results/defaults_center_hold_color_hops_lt.txt), [`defaults_center_hold_color_hops_le.txt`](apa-kan/game/benchmark_results/defaults_center_hold_color_hops_le.txt). After scoring hold terminals from the mover: [`defaults_center_hold_color_hops.txt`](apa-kan/game/benchmark_results/defaults_center_hold_color_hops.txt), plus petal-majority N=1 [`defaults_plus_petal_majority_n1.txt`](apa-kan/game/benchmark_results/defaults_plus_petal_majority_n1.txt), petal-majority N=3 instead of center-hold [`petal_majority_n3_no_center_hold.txt`](apa-kan/game/benchmark_results/petal_majority_n3_no_center_hold.txt), and new vs legacy [`minimax_new_vs_legacy.txt`](apa-kan/game/benchmark_results/minimax_new_vs_legacy.txt). Control-win (center-hold off) at t=1 [`control_win_t1.txt`](apa-kan/game/benchmark_results/control_win_t1.txt), t=3 [`control_win_t3.txt`](apa-kan/game/benchmark_results/control_win_t3.txt), t=1 + insufficient-tiles strict [`control_win_t1_strict.txt`](apa-kan/game/benchmark_results/control_win_t1_strict.txt), t=1 + no-recapture [`control_win_t1_no_recapture.txt`](apa-kan/game/benchmark_results/control_win_t1_no_recapture.txt). Same three with `--starting-bench 7`: [`control_win_t1_bench7.txt`](apa-kan/game/benchmark_results/control_win_t1_bench7.txt) (now the default package: control-win t=1, center-hold off, bench 7, insufficient-tiles off), [`control_win_t1_bench7_strict.txt`](apa-kan/game/benchmark_results/control_win_t1_bench7_strict.txt), [`control_win_t1_bench7_no_recapture.txt`](apa-kan/game/benchmark_results/control_win_t1_bench7_no_recapture.txt). Deck fairness at 12 games/pairing/seat on those defaults: [`deck_fairness_default.txt`](apa-kan/game/benchmark_results/deck_fairness_default.txt) (same/mirrored bags), [`deck_fairness_independent.txt`](apa-kan/game/benchmark_results/deck_fairness_independent.txt), [`deck_fairness_complementary.txt`](apa-kan/game/benchmark_results/deck_fairness_complementary.txt), [`deck_fairness_complementary_swapped.txt`](apa-kan/game/benchmark_results/deck_fairness_complementary_swapped.txt). Six-color multi-ring default vs `--max-rings 1` (release, 12/pairing/seat, 1000 ms): [`multi_ring_default.txt`](apa-kan/game/benchmark_results/multi_ring_default.txt), [`multi_ring_single.txt`](apa-kan/game/benchmark_results/multi_ring_single.txt). Zags on (1/7) vs zags off under graph hops: [`zag_rings_default.txt`](apa-kan/game/benchmark_results/zag_rings_default.txt), [`zag_rings_off.txt`](apa-kan/game/benchmark_results/zag_rings_off.txt). Counted pass-through (now the default) GE vs GT, zags 1/7 and off: [`pass_through_ge_default.txt`](apa-kan/game/benchmark_results/pass_through_ge_default.txt), [`pass_through_ge_zags_off.txt`](apa-kan/game/benchmark_results/pass_through_ge_zags_off.txt), [`pass_through_gt_default.txt`](apa-kan/game/benchmark_results/pass_through_gt_default.txt), [`pass_through_gt_zags_off.txt`](apa-kan/game/benchmark_results/pass_through_gt_zags_off.txt). Undirected zag-mismatch SUM vs MAX (pass-through GE, zags 1/7): [`zag_mismatch_sum.txt`](apa-kan/game/benchmark_results/zag_mismatch_sum.txt), [`zag_mismatch_max.txt`](apa-kan/game/benchmark_results/zag_mismatch_max.txt). Capture budget + low-value win on that SUM default: [`capture_budget_low_value.txt`](apa-kan/game/benchmark_results/capture_budget_low_value.txt). Smaller generative bags (7 dealt) × self-capture budget on/off: [`deck11_self_capture_on.txt`](apa-kan/game/benchmark_results/deck11_self_capture_on.txt), [`deck11_self_capture_off.txt`](apa-kan/game/benchmark_results/deck11_self_capture_off.txt), [`deck13_self_capture_on.txt`](apa-kan/game/benchmark_results/deck13_self_capture_on.txt), [`deck13_self_capture_off.txt`](apa-kan/game/benchmark_results/deck13_self_capture_off.txt). Usable budget (`min(tokens, bench)`) rerun of that grid: [`usable_budget_bag27.txt`](apa-kan/game/benchmark_results/usable_budget_bag27.txt), [`usable_budget_deck11_on.txt`](apa-kan/game/benchmark_results/usable_budget_deck11_on.txt), [`usable_budget_deck11_off.txt`](apa-kan/game/benchmark_results/usable_budget_deck11_off.txt), [`usable_budget_deck13_on.txt`](apa-kan/game/benchmark_results/usable_budget_deck13_on.txt), [`usable_budget_deck13_off.txt`](apa-kan/game/benchmark_results/usable_budget_deck13_off.txt). Progressive lowest-tile pair check on bag 11 refund-on: [`low_value_pairs_deck11_on.txt`](apa-kan/game/benchmark_results/low_value_pairs_deck11_on.txt). Settled defaults (bag 11, self-capture on, usable budget, pair-wise low-value; no extra rule flags): [`defaults_bag11.txt`](apa-kan/game/benchmark_results/defaults_bag11.txt). Earlier optional-pass-through logs: [`counted_pass_through_default.txt`](apa-kan/game/benchmark_results/counted_pass_through_default.txt), [`counted_pass_through_off.txt`](apa-kan/game/benchmark_results/counted_pass_through_off.txt). The earlier 20 ms round-robin is in [`apa-kan/game/benchmark_results/round_robin.txt`](apa-kan/game/benchmark_results/round_robin.txt).

## Test

```bash
RUSTFLAGS=-Dwarnings cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all -- --check
```

## License

Copyright © 2026 Ramate LLC. All rights reserved.

This software is made available under the Apa Kan Source-Available License. See [LICENSE.md](LICENSE.md) for details.
