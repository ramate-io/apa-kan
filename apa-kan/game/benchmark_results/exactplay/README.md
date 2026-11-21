# Exactplay bots for live braid-only-exact + double-count

Stacked on PR #83 (live playtest `RuleSet`: 4:3:1 Additive, `braid-only-exact`,
single braids on, normal-only SWAP, center↔inner double-count, color-drift
160). That PR’s confirmation cell
[`h2h_defaults`](../single_braid_exact_swap/h2h_defaults.txt) is the **old
bot** baseline: `--search-profile counterplay`, `--pick search`. Search pick
there dumps 1-rings into the unused bag (9.6% of opening benches); minimax-as-P1
sits; MCTS searches ~1.75 ply; CLAIM is ~0; late games walk at budget 0.

This package does **not** change rules. It adds saveable `SearchConfig` knobs
(eval / qsearch / ordering / pick) and a new **`exactplay`** personality
([`exactplay.toml`](../../bots/exactplay.toml), `--search-profile exactplay`)
so bots can contest the 9-node control set under the live scoring.

Reproduce with [`run_h2h.sh`](run_h2h.sh). Survey: [`survey.py`](survey.py)
(first center *take* counts only if the landing piece is still there after
that action’s captures resolve). Do not overwrite these `h2h_*` logs.

## What changed

New knobs default to 0 / off, so `enhanced` / `counterplay` / committed TOML
personalities are bitwise unchanged. `exactplay()` starts from `counterplay`
and retunes:

| Lever | `counterplay` | `exactplay` | Why |
|---|---|---|---|
| `eval_center_occ` / `eval_inner_ring` / `eval_center_pressure` | 70 / 70 / 100 | 50 / 90 / 120 | Sit less on origin; race the 8 inner nodes |
| `eval_braid_fulfill` / `eval_occupy_block` | 0 / 0 | 14 / 8 | Exact braid slots; non-braid matches occupy/paint, score 0 |
| `eval_mismatch_control` / `eval_control_race` | 0 / 0 | 80 / 80 | Mismatch tax on the 9-node set; distance to control-5 |
| `eval_claim` / `eval_budget` | 0 / 0 | 36 / 50 | Local-claim overwhelmed tiles; usable tokens |
| `eval_single_braid` / `eval_swap_piece` / `eval_clean_pop` | 0 / 0 / 0 | 40 / 28 / 220 | 1-slot exact +1; 1-normal SWAP initiator; non-mutual pop threat |
| `q_depth` / `q_include_swaps` / `q_include_braid_hits` | 4 / off / off | 6 / on / on | Qsearch SWAPs and 1-slot exact landings, not only captures |
| `order_swap` / `order_single_braid` / `order_claim` / `order_zero_budget_walk` | 0 | 500 / 90 / 240 / 180 | Prefer steals and braid hits; tax inner-ring walking |
| `pick_single_braid` / `pick_single_normal` / `pick_one_ring` | 0 / 0 / 0 | 48 / 32 / 10 | Keep 1-slot braids and SWAP-normals on the 11 |
| MCTS priors / prior-widening / solver / PUCT | off / off / off / 2.0 | on / on / on / 1.7 | Spend 650 iters on ordered pops, not sitting |

`--search k=v` overlays any of these. `--p1-search-profile` /
`--p2-search-profile` (bench) and `SetupConfig.p1_search` / `p2_search`
make pick personality per-seat, so mixed H2H drafts the way each bot would.

## Package

Release `apa-kan-bench`, `--no-default-features`, `--budget nodes:33000,iters:650`,
`--games 50`, `--jobs 4`, `--no-depth-probe`, `--zags on`, `--zag-mismatch sum`,
`--endgame-rule control-cascade`, `--cascade-turn-gain on`,
`--capture-mode local-claim`, `--no-same-piece-twice off`, `--pick search`,
`--pairings minimax:mcts,mcts:minimax`, `--same-deck on`, `--seed 1`.
Live `RuleSet::default()` (no extra rule flags). 50 games × 2 pairings =
**100 games** per cell.

| Log | P1 profile | P2 profile | Notes |
|---|---|---|---|
| [`h2h_self`](h2h_self.txt) | exactplay | exactplay | improved vs improved; center stats |
| [`h2h_exact_as_p1`](h2h_exact_as_p1.txt) | exactplay | counterplay | improved vs old, both engines |
| [`h2h_exact_as_p2`](h2h_exact_as_p2.txt) | counterplay | exactplay | old vs improved, both engines |
| [#83 live defaults](../single_braid_exact_swap/h2h_defaults.txt) | counterplay | counterplay | old vs old on the same rules |

JSONL.gz logs reconstruct each game (rules include `braid_scoring`,
`allow_single_braids`, `swap_mode`, `score_unique_neighbors`; opening benches
and per-action search depth). Same-deck 27 identical=100/100 on every log.

## Head-to-head

First take is MOVE in every recorded take (none by ADD or SWAP). CLAIM stays
~0.02/game, ~0 on center.

| Log | P1 W-D-L | Turns | Ends (lock/ctrl/other) | First take | Never left | First-taker wins | Opp cap | Mutual | Holder→inner / reverse | Swaps | ZB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| #83 **counterplay** | 59-0-41 | 53.93 | 28 / 66 / 6 | 96 @ t10.16 | 26.0% | 51.0% | 0.82 | 90.2% | 1.35 / 0.84 | 0.16 | 27.1% |
| **exactplay self** | **49-1-50** | **46.07** | **39** / 47 / 14 | 96 @ t10.30 | **11.5%** | **49.0%** | **1.31** | 91.6% | 2.15 / 0.70 | **1.51** | **13.7%** |
| exact P1 vs counter P2 | 46-0-54 | 58.61 | 31 / 62 / 7 | 98 @ t10.01 | 13.3% | 48.0% | 0.98 | 87.8% | 1.82 / 1.01 | 0.74 | 24.2% |
| counter P1 vs exact P2 | **64-1-35** | 55.79 | 36 / 54 / 10 | 95 @ t10.47 | **9.5%** | 47.4% | 1.16 | 83.6% | 1.85 / 0.65 | 0.88 | 27.5% |

Contested-center / sit-and-win (among games with a surviving take):

| Log | Contested | Sit-and-win | Center swaps/g | Dislodging swaps/g | 1-braid center pops/g |
|---|---|---|---|---|---|
| #83 counterplay | 74.0% | 22.9% | 0.10 | 0.07 | 0.020 |
| exactplay self | **88.5%** | **10.4%** | **0.81** | **0.71** | **0.140** |
| exact P1 vs counter P2 | 85.7% | 13.3% | 0.28 | 0.17 | 0.040 |
| counter P1 vs exact P2 | **89.5%** | **8.4%** | 0.41 | 0.22 | 0.100 |

Control-cascade target5 / 4 / 3 / 2 (immediate vs hold), exactplay self:
16 / 3 / 11 / 17, immediate 17, hold 30. More lockouts (39 vs 28) and fewer
control-5 holds than #83.

**Self-play contests the center.** Never-left 26% → 11.5%, sit-and-win 23% →
10%, swaps 0.16 → 1.51 (81 center SWAPs in 100 games), zero-budget share
27% → 14%, P1 seat 59-0-41 → 49-1-50. Cost: lockouts 28 → 39, and the
non-holder’s inner captures stay below the holder’s (0.70 vs 2.15).

**Old counterplay still wins the mixed H2H.** Across both seats, exactplay
is 81-1-118 in 200 games (~40.5%). Sitting + 2–3-ring benches beat a
singles-heavy contester on this shallow budget. That is the open strength
question, not a rule bug.

## By pairing / seat

Minimax-as-P1 still beats MCTS. Exactplay made MCTS-as-P1 *worse* vs
minimax (14-0-36 vs #83’s 27-0-23).

| Log | mcts vs minimax P1 | never / first_wins / opp / swaps | minimax vs mcts P1 | never / first_wins / opp / swaps |
|---|---|---|---|---|
| #83 counterplay | 27-0-23 | 18.8% / 39.6% / 0.88 / 0.10 | 32-0-18 | 33.3% / 62.5% / 0.76 / 0.22 |
| exactplay self | **14-0-36** | **12.5%** / 39.6% / 1.26 / 1.82 | **35-1-14** | **10.4%** / 58.3% / 1.36 / 1.20 |
| exact P1 vs counter P2 | 19-0-31 | 16.0% / 42.0% / 0.98 / 0.64 | 27-0-23 | 10.4% / 54.2% / 0.98 / 0.84 |
| counter P1 vs exact P2 | 26-0-24 | 6.7% / 33.3% / 1.12 / 0.70 | **38-1-11** | 12.0% / 60.0% / 1.20 / 1.06 |

Same-engine cross-seat (exact vs counter):

- Exact **minimax** vs counter **MCTS**: 27-0-23 as P1 + 24-0-26 as P2 → **51-0-49**. Even.
- Exact **MCTS** vs counter **minimax**: 19-0-31 as P1 + 11-1-38 as P2 → **30-1-69**. The
  old minimax sitter crushes the new MCTS.

Search depth did not move: exactplay self `mm_avg=2.90` (was 3.53),
`mcts_avg=1.81` (was 1.77). Deeper qsearch ate ID plies; 650 MCTS iters with
priors still average ~1.8 visit-weighted ply.

## Draft and 1-ring lives

Opening 11-tile benches vs 16-tile reserves. #83 search pick **hides**
single braids; exactplay **keeps** them (and then some).

| Log | Seat | 1-ring bench | Single-braid bench | Single-normal bench | 1-ring bag |
|---|---|---|---|---|---|
| #83 counterplay | both | **9.6%** | **4.4%** | **2.5%** | 75.0% |
| exactplay self | both | **52.3%** | **29.6%** | **22.4%** | 45.7% |
| mixed H2H | exactplay | 50.4% | 27.5% | 22.2% | 47.0% |
| mixed H2H | counterplay | 11.4% | 4.4% | 3.1% | 73.8% |

Placed 1-ring lives (exactplay self vs #83 defaults):

| Kind | Log | Placed | Moved later | Survived | Touched control | Swapped |
|---|---|---|---|---|---|---|
| single-normal | #83 | 29 | 62.1% | 20.7% | 51.7% | 20.7% |
| single-normal | exact self | **316** | 63.0% | 14.2% | 53.8% | 21.2% |
| single-braid | #83 | 74 | 56.8% | 23.0% | 35.1% | 0% |
| single-braid | exact self | **494** | 65.8% | 24.1% | **55.1%** | 0% |

Single braids never SWAP (normal-only is doing its job). They now *reach
play*: 494 placed vs 74, 14 center opponent-pops vs 2. Every one of those
14 pops was a **mutual** trade — the “hit the center without losing a
single” line did not show up as a clean pop in 100 self-play games. The 11
clean (non-mutual) center pops were almost all 3-ring MOVEs plus one SWAP.

Pick overshoots. Generation is ~49.5% 1-ring / 18.7% single-braid; exactplay
puts **52%** 1-ring on the 11 (about six singles). `tile_score` plus
`set_role_bonus` (first copy full knob, extras half) outbids 3-rings. A
human drafter would keep one or two, not half the bench. That is the leading
suspect for the H2H loss: heavies-heavy counterplay vs singles-heavy
exactplay.

## Qualitative log review

Citations are `log` / `pairing` / `game` / `turns`. Inner-ring node ids are
`{9,1,12,3,16,6,18,21}`; center is 0. Every first take in these 300 games is
a MOVE. CLAIM-on-center is 0.

### SWAP is now a real steal

[`h2h_self.jsonl.gz`](h2h_self.jsonl.gz) / mcts vs minimax / game 16 / 34
turns. P1 ADDs a 1-braid on 8 at t3 and MOVEs 8→0 at t7 (first take with a
single braid). P2’s matching 1-braid MOVEs 17→18 at t8 and pops it (mutual).
P2 retakes 11→0 at t10 with another 1-braid. **P1 SWAP 3→0 at t13** (1-normal
exchanges with P2’s 1-braid — a dislodge without a capture). P2 immediately
pops from inner 6 at t14. This is the grammar #83 wanted and almost never
saw (5 dislodging SWAPs in 100 defaults). Exactplay self-play has 71.

A SWAP war:
[`h2h_self.jsonl.gz`](h2h_self.jsonl.gz) / mcts vs minimax / game 20 / 56
turns. P2 takes with a 1-braid at t4 (self-pop on landing); retakes with a
3-normal at t10. P1 SWAP 3→0 at t17 (1-normal vs 3-normal); P2 SWAP 1→0 at
t18; P1 SWAP 1→0 at t21; P1 SWAP 0→3 at t23 dropping the 3-normal back;
then t27 SWAP 3→0 again. P2 pops from 18 at t28. Later P2 SWAP 6→0 / 0→1
shuffles 1-normals on the origin. Control-5 hold. The origin is a SWAP
hot-potato, not a sit.

### Single-braid hits are mutual trades

[`h2h_self.jsonl.gz`](h2h_self.jsonl.gz) / mcts vs minimax / game 7 / 42
turns. P1 takes t11 with a 3-ring; P2 pops from inner 6 at t12 (mutual, also
takes P1’s 1-braid on 8). P1 retakes t15; **P2 1-braid 14→16 pops center at
t16** (mutual). P1 1-braid onto 0 at t21 (budget 0); P2 1-braid 11→12 pops
at t22 (mutual). Three center trades in 11 turns, two of them 1-braid hits,
all mutual. Lockout. Liam’s “hit without losing a single” is legal; the
eval still walks the braid *onto* a mismatch stack.

### Sit-and-win is rarer, not gone

[`h2h_self.jsonl.gz`](h2h_self.jsonl.gz) / mcts vs minimax / game 1 / 38
turns. P2 MOVEs 2→0 at t6 with a 3-normal and never leaves. P1 never reaches
an adjacent hitch (ADD 1-normal on 23, walks to 21, self-pops; later SWAPs
are outer-ring). Lockout at control-5. This is still ~10% of takes.

### CLAIM is still not recapture

Two CLAIMs in 100 self-play games, neither on center.
[`h2h_self.jsonl.gz`](h2h_self.jsonl.gz) / minimax vs mcts / game 3 / 92
turns. P2 CLAIMs own 1-braid on 17 at t86 (heal), then ADDs for a cascade
hold. Inner-adjacent MOVE and SWAP remain the recapture grammar.

### Zero-budget walking is shorter, not dead

Exactplay self ZB 13.7% (was 27%). The long exception is still walking:
[`h2h_self.jsonl.gz`](h2h_self.jsonl.gz) / minimax vs mcts / game 49 / 200
turns, draw on turn-limit. P2 takes t6; P1 pops from inner 12 at t7; P1
1-braid takes t13; then a 3-braid walks 0↔inner from t55 to t200 at budget
0 (22 holders, 19 vacates). `order_zero_budget_walk` does not tax MOVE *onto*
the vacant center, so the hot-potato is the tax-free line.

### Mixed H2H: heavies sit, singles SWAP

[`h2h_exact_as_p1.jsonl.gz`](h2h_exact_as_p1.jsonl.gz) / minimax vs mcts /
game 3 / 145 turns. Exactplay minimax P1 (singles bench) vs counterplay MCTS
P2. P1 1-braid onto 0 at t29; later SWAP 6→0 at t99 (budget 0) then is
popped from inner 3 at t100. P2 (heavies) lockout-wins. The new toys fire;
the old bench wins.

Counterplay minimax-as-P1 vs exactplay MCTS is the blowout (38-1-11): the
sitter with 2–3 rings lockouts the singles contester in 23–47 turns
(e.g. cell `h2h_exact_as_p2` / minimax vs mcts / games 44–45).

## Open design questions

- **Pick overshoots.** 52% 1-ring on the 11 vs #83’s 9.6% dump. The knobs
  plus `set_role_bonus` (first copy full, extras half) outbid 3-rings. Keep
  one 1-braid and one 1-normal, or keep half the bench? A taper cap of 1–2
  is the obvious next overlay (`pick-single-braid=20,pick-one-ring=0`?).
- **H2H vs sitters.** Exactplay self-play is the counterplay cell (never-left
  11.5%, first-taker 49%). Mixed with counterplay it loses 81-1-118. Is
  “contest the center” the goal, or “beat the sitter on this budget”? Those
  diverged: sitting on a 3-ring is still a winning reply to a singles rush.
- **Minimax-as-P1.** Still 35-1-14 in exactplay self. Lower `eval_center_occ`
  (50 vs 70) did not stop the seat; it stopped the sit (never-left 10% vs
  33%). The P1 edge is search, not occupancy.
- **MCTS ~1.8 ply.** Priors + solver + PUCT 1.7 did not deepen the tree at
  650 iters. Exactplay MCTS-as-P1 is *weaker* vs minimax than counterplay
  MCTS was. Drop priors? Raise iters? `playout_leaf_depth=1`?
- **Qsearch ate ID depth.** `q_depth=6` plus every SWAP as tactical dropped
  minimax completed depth 3.53 → 2.90. Worth it for 0.71 dislodging SWAPs/g,
  or cap `q_include_swaps` to center SWAPs only?
- **Single-braid pops are mutual.** 14/14 in self-play. The eval walks the
  1-braid onto the origin; Additive mismatch then pops both. A “clean pop”
  bonus of 220 did not produce the non-trade hit. Need a quiet-landing
  constraint, or accept trades?
- **CLAIM still dead** (~0.02/game, 0 on center, the two self-play CLAIMs
  are heals). `eval_claim=36` and `order_claim=240` did not make it a
  recapture. Overwhelm is still resolved by MOVE/SWAP, not CLAIM.
- **Lockouts 39%.** Forcing a reaction costs lockouts again (28 → 39).
  Acceptable for playtesting?
- **Zero-budget onto vacant center** is tax-free, so the 200-turn
  hot-potato remains (game 49). Tax vacate-and-retake? A threefold draw?
- **Live bot default.** UI / `cargo run` still uses `enhanced` unless
  `--search-profile exactplay`. Flip the playtest default to exactplay, keep
  counterplay as the sitter, or wait on a milder pick overlay?
