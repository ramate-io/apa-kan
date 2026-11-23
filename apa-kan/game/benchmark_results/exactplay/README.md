# Exactplay bots: eval-only leaf margin for braid-only-exact

Live `RuleSet` (PR #88) is ratchet reward + 2-2-1-1 on the #83 scoring
package (4:3:1 Additive, `braid-only-exact`, single braids on, normal-only
SWAP, center↔inner double-count, color-drift 160). That PR’s confirmation
cell [`h2h_defaults`](../single_braid_exact_swap/h2h_defaults.txt) is the
**old bot** baseline: `--search-profile counterplay`, `--pick search`.

Committed H2H, isolation, deep, distill, and wide logs in this directory
were recorded **before** the #88 lock (1-1-1-1 seating, no ratchet).
`mcts_root_noise` defaults to 0 and minimax shuffles the root only when
`ordering=off`, so live `exactplay` stays bit-identical.

This package does **not** change rules. It does **not** program a playstyle
(no pick-single-braid knobs, extra qsearch, SWAP/braid move-order hacks, or
MCTS priors toward center contest). The existing search tree is unchanged.
The only new lever is `eval_margin_scale`, a percent scale on the leaf
`8 * (matches − mismatches)` term from `Game::tile_score` (100 = enhanced;
`exactplay` = 200) so braid-only-exact scoring — fulfilled braids only,
1-slot exact +1, occupy-without-score already at match 0 — is not drowned
by material/geometry.

[`exactplay.toml`](../../bots/exactplay.toml) / `--search-profile exactplay`
starts from `enhanced`, not `counterplay`. Missing TOML keys still take
`enhanced` defaults. `counterplay` is bitwise unchanged.

Reproduce with [`run_h2h.sh`](run_h2h.sh) (vs counterplay),
[`run_isolation.sh`](run_isolation.sh) (vs enhanced + cheap counter vs
enhanced), [`tune_eval.py`](tune_eval.py) (coordinate descent),
[`run_deep.sh`](run_deep.sh) (4x/8x budget probe),
[`run_distill.sh`](run_distill.sh) (cheap_order fit from 4x roots),
[`run_explore.sh`](run_explore.sh) (deep wide vs standard 4x/8x),
[`audit_fairness.py`](audit_fairness.py), and
[`analyze_wide.py`](analyze_wide.py). Survey:
[`survey.py`](survey.py) (first center *take* counts only if the landing
piece is still there after that action’s captures resolve).

## What changed

`eval_margin_scale` defaults to 100, so `enhanced` / `counterplay` / committed
TOML personalities keep the same leaf. `exactplay()` is `enhanced()` with
`eval_margin_scale = 200`. Search-pick rollouts use the seat’s
`evaluate_with` (so the scale applies when `--pick search`); heuristic/greedy
pick has no personality bonuses.

| Lever | `enhanced` / `counterplay` | `exactplay` | Why |
|---|---|---|---|
| `eval_margin_scale` | 100 | 200 | Restore braid-only-exact `tile_score` against geometry |
| qsearch / `is_tactical` | captures + center SWAP | same | No extra SWAP-all or braid-hit subsearch |
| MCTS priors / PW / solver | off (enhanced) | off | No scripted move-type priors |
| pick-single-braid / one-ring | n/a (removed) | n/a | No drafted playstyle |
| `eval_center_occ` / inner / pressure | 180 / 55 / 0 (enhanced); 70 / 70 / 100 (counterplay) | **enhanced** 180 / 55 / 0 | Geometry knobs stay on the existing tree |

`--search eval-margin=N` overlays the scale (clamped 1..=400; 0 → 100).
`--p1-search-profile` / `--p2-search-profile` keep per-seat eval for mixed
H2H, including search-pick rollouts.

200 started as a scoring-faithful guess: braid-only match totals are a subset
of mixed-channel matches, so the ×8 term shrank relative to material.
Coordinate descent over the four existing `SearchConfig` eval knobs (seed 7,
40-game neighbor samples) **did not promote a neighbor** of
`200 / 180 / 55 / 0`. Isolation vs `enhanced` (scale 200 vs 100, same
geometry) is **108-2-90**. The earlier 141-1-58 vs `counterplay` is mostly
`eval_center_occ` 180 vs 70, not the scale.

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
| [`h2h_self`](h2h_self.txt) | exactplay | exactplay | improved vs improved |
| [`h2h_exact_as_p1`](h2h_exact_as_p1.txt) | exactplay | counterplay | improved vs old |
| [`h2h_exact_as_p2`](h2h_exact_as_p2.txt) | counterplay | exactplay | old vs improved |
| [`h2h_exact_vs_enh_p1`](h2h_exact_vs_enh_p1.txt) | exactplay | enhanced | scale 200 vs 100, same geometry |
| [`h2h_exact_vs_enh_p2`](h2h_exact_vs_enh_p2.txt) | enhanced | exactplay | scale 100 vs 200 |
| [`h2h_counter_vs_enh_p1`](h2h_counter_vs_enh_p1.txt) | counterplay | enhanced | 50 games; geometry 70 vs 180 |
| [`h2h_counter_vs_enh_p2`](h2h_counter_vs_enh_p2.txt) | enhanced | counterplay | 50 games |
| [#83 live defaults](../single_braid_exact_swap/h2h_defaults.txt) | counterplay | counterplay | old vs old on the same rules |
| [`h2h_deep4x_self`](h2h_deep4x_self.txt) | exactplay 4x | exactplay 4x | `nodes:132000,iters:2600` both seats |
| [`h2h_deep4x_vs_light_p1`](h2h_deep4x_vs_light_p1.txt) | exactplay 4x | exactplay light | deep P1 vs light P2 |
| [`h2h_deep4x_vs_light_p2`](h2h_deep4x_vs_light_p2.txt) | exactplay light | exactplay 4x | light P1 vs deep P2 |
| [`h2h_deep8x_self`](h2h_deep8x_self.txt) | exactplay 8x | exactplay 8x | `nodes:264000,iters:5200` both seats |
| [`distill/`](distill/) | light re-search of 4x roots | — | `cheap_order` fit; champion unchanged |
| [`explore/h2h_wide4_p1`](explore/h2h_wide4_p1.txt) | wide 4x | exactplay 4x | seed 1, 10 games |
| [`explore/h2h_wide4_p2`](explore/h2h_wide4_p2.txt) | exactplay 4x | wide 4x | seed 1, 10 games |
| [`explore/h2h_wide4_p1_s11`](explore/h2h_wide4_p1_s11.txt) | wide 4x | exactplay 4x | seed 11, 6 games |
| [`explore/h2h_wide4_p2_s11`](explore/h2h_wide4_p2_s11.txt) | exactplay 4x | wide 4x | seed 11, 6 games |
| [`explore/h2h_wide8_p1`](explore/h2h_wide8_p1.txt) | wide 8x | exactplay 8x | seed 1, 4 games |
| [`explore/h2h_wide8_p2`](explore/h2h_wide8_p2.txt) | exactplay 8x | wide 8x | seed 1, 4 games |

JSONL.gz logs reconstruct each game. Same-deck 27 identical=100/100 on every
log. Search labels show `eval_margin=200`, `q=on/4`, priors off, `eval_center=180`.

## Head-to-head

First take is MOVE in every recorded take (none by ADD or SWAP). CLAIM is 0
in all 300 games.

| Log | P1 W-D-L | Turns | Ends (lock/ctrl/other) | First take | Never left | Sit-and-win | Center rem/g | Swaps/g | ZB |
|---|---|---|---|---|---|---|---|---|---|
| #83 **counterplay** | 59-0-41 | 53.93 | 28 / 66 / 6 | 96 @ t10.16 | 26.0% | 22.9% | 0.82 | 0.16 | 27.1% |
| **exactplay self** | **48-2-50** | 58.41 | **24** / **71** / 5 | 93 @ t10.73 | **24.7%** | **22.6%** | 0.47 | 0.02 | 27.1% |
| exact P1 vs counter P2 | **71-0-29** | 54.37 | 31 / 69 / 0 | 88 @ t10.49 | 28.4% | 18.2% | 0.57 | 0.10 | 26.7% |
| counter P1 vs exact P2 | **29-1-70** | 59.51 | 28 / 66 / 6 | 96 @ t9.54 | 32.3% | 20.8% | 0.58 | 0.03 | 28.2% |

Center rem/g is opponent tiles removed from node 0. Sit-and-win is
never-left *and* the first-taker’s side won, among games with a surviving
take. CLAIM 0.00/game on every cell.

**Self-play looks like the old bot, not a scripted contester.** Never-left
26% → 24.7%, sit-and-win 23% → 23%, 1-ring benches stay ~9.5%, SWAP 0.16 →
0.02, zero-budget share stays 27%, ID depth stays ~3.5 (tactical qsearch
had dropped it to 2.90). P1 seat 59-0-41 → 48-2-50. Center pops are *lower*
(0.82 → 0.47): the leaf did not invent a contest-the-origin line.

**Eval-only exactplay still beats counterplay from both seats.** Combined
H2H: exactplay **141-1-58** in 200 games. The previous tactic-heavy
exactplay lost 81-1-118 by drafting 52% 1-rings. Here the benches match
#83 (~9.5% 1-ring); the gap is the leaf, not a forced playstyle.

## By pairing / seat

Minimax-as-P1 is no longer a blowout in self-play (25-0-25). Mixed with
counterplay, exactplay minimax is the large edge; exactplay MCTS still wins.

| Log | mcts vs minimax P1 | never / first_wins / opp / swaps | minimax vs mcts P1 | never / first_wins / opp / swaps |
|---|---|---|---|---|
| #83 counterplay | 27-0-23 | 18.8% / 39.6% / 0.88 / 0.10 | 32-0-18 | 33.3% / 62.5% / 0.76 / 0.22 |
| exactplay self | **23-2-25** | 22.7% / 40.9% / 0.46 / 0.04 | **25-0-25** | 26.5% / 59.2% / 0.48 / 0.00 |
| exact P1 vs counter P2 | 27-0-23 | 32.6% / 41.3% / 0.68 / 0.14 | **44-0-6** | 23.8% / 52.4% / 0.46 / 0.06 |
| counter P1 vs exact P2 | **12-1-37** | 34.0% / 55.3% / 0.50 / 0.00 | **17-0-33** | 30.6% / 34.7% / 0.66 / 0.06 |

Same-engine cross-seat (exact vs counter):

- Exact **minimax** vs counter **MCTS**: 44-0-6 as P1 + 37-1-12 as P2 → **81-1-18**.
- Exact **MCTS** vs counter **minimax**: 27-0-23 as P1 + 33-0-17 as P2 → **60-0-40**.

Search depth did not move: exactplay self `mm_avg=3.48` (was 3.53 on #83;
tactical exactplay was 2.90), `mcts_avg=1.78` (was 1.77). Same tree, same
budget.

## Draft and 1-ring lives

Opening 11-tile benches vs 16-tile reserves. Search pick still **hides**
1-rings, same as #83 — we did not add pick bonuses.

| Log | Seat | 1-ring bench | Single-braid bench | Single-normal bench | 1-ring bag |
|---|---|---|---|---|---|
| #83 counterplay | both | 9.6% | 4.4% | 2.5% | 75.0% |
| exactplay self | both | **9.5%** | **4.3%** | **2.5%** | 75.1% |
| mixed H2H | exactplay | 9.5% | 4.3% | 2.5% | 75.1% |
| mixed H2H | counterplay | 9.8% | 4.5% | 2.5% | 74.9% |

Placed 1-ring lives (exactplay self vs #83 defaults):

| Kind | Log | Placed | Moved later | Survived | Touched control | Swapped |
|---|---|---|---|---|---|---|
| single-normal | #83 | 29 | 62.1% | 20.7% | 51.7% | 20.7% |
| single-normal | exact self | 31 | 61.3% | 16.1% | 54.8% | 6.5% |
| single-braid | #83 | 74 | 56.8% | 23.0% | 35.1% | 0% |
| single-braid | exact self | 63 | 58.7% | 20.6% | 44.4% | 0% |

Single-braid center opponent-pops: **0** in 100 self-play games (tactical
exactplay had 14, all mutual). Singles never SWAP (normal-only). Generation
is still ~49.5% 1-ring / 18.7% single-braid in the 27; search pick keeps
~9.5% on the 11.

## Qualitative (self-play and vs counterplay)

Citations are `log` / `pairing` / `game` / `turns`. Inner-ring node ids are
`{9,1,12,3,16,6,18,21}`; center is 0. Every first take in these 300 games is
a MOVE. CLAIM is 0. Win paths vs enhanced after isolation are in the
section after the hillclimb.

### Sit-and-win is still the control-5 line

[`h2h_self.jsonl.gz`](h2h_self.jsonl.gz) / mcts vs minimax / game 1 / 32
turns. P2 MOVEs 5→0 at t18 with a 2-normal and never leaves. Control-5 hold.
No SWAP, no CLAIM, no center pop. This is 21/93 takes (22.6%), matching
#83’s 22.9% — the leaf scale did not script a contest.

### SWAP is rare without a tactic knob

Two SWAPs in 100 self-play games.
[`h2h_self.jsonl.gz`](h2h_self.jsonl.gz) / mcts vs minimax / game 23 / 97
turns. P2 takes t12 with a 3-ring, walks off and back at budget 0, P1 pops
the occupant at t57 (mutual with an inner 3). At t76 P1 SWAP 18→0 with a
1-normal onto its own 2-ring that landed t74 — an own-rotate, not a steal.
The other SWAP (game 40, 5→6) never touches center. Dislodging SWAPs/game:
0.00. The old tree already orders center SWAP first; it does not *find*
them at this budget unless we extend qsearch (which we did not).

### Zero-budget walking still hits the 200-turn cap

[`h2h_self.jsonl.gz`](h2h_self.jsonl.gz) / mcts vs minimax / game 11 / 200
turns, draw on turn-limit. P1 MOVEs 14→0 at t3 with a 3-ring, vacates 0→8
at t43. P2 takes 18→0 at t44, then from t186 walks 0↔inner at budget 0
(self-pop on 188). No `order_zero_budget_walk` tax: that was a playstyle
hack. ZB share 27.1%, same as #83.

### Mixed H2H: scoring-faithful sit beats counterplay’s “don’t sit”

[`h2h_exact_as_p2.jsonl.gz`](h2h_exact_as_p2.jsonl.gz) / mcts vs minimax /
game 1 / 28 turns. Counterplay MCTS is P1; exactplay minimax is P2. P2
MOVEs 23→0 at t8 with a 2-normal and never leaves. Control-5 hold. Exactplay
as P2 sits and wins — the high `eval_center_occ` (180, from enhanced) plus
scaled `tile_score` values a healthy occupant more than counterplay’s
downweighted center (70) does.

The blowout seat is exactplay minimax as P1:
[`h2h_exact_as_p1.jsonl.gz`](h2h_exact_as_p1.jsonl.gz) / minimax vs mcts /
game 1 / 29 turns. P1 MOVEs 2→0 at t7 with a 3-ring (inner cap on 21),
vacates 0→11 at t17, P2 retakes 8→0 at t26, then P1 lockout-wins. Not a
sit; a short cascade. That pairing is 44-0-6.

Counterplay minimax-as-P1 vs exactplay MCTS is no longer the sitter crush
(it was 38-1-11 under tactics). Here it is 17-0-33: exactplay MCTS as P2
wins.

## Isolation: scale vs geometry

Same light setup as the 100-game cells (`run_isolation.sh`, `--seed 1`).
Exactplay vs enhanced is 100 games each seat. Counterplay vs enhanced is
`--games 25` × 2 pairings = 50 games each seat (cheap geometry check).

| Log | P1 W-D-L | Turns | Lock / ctrl | Never left | Sit-and-win | Center rem/g | Swaps/g | CLAIM | mm / mcts |
|---|---|---|---|---|---|---|---|---|---|
| exact P1 vs enh P2 | **59-1-40** | 54.34 | 31 / 64 | 27.0% | 24.7% | 0.57 | 0.40† | 0 | 3.48 / 1.80 |
| enh P1 vs exact P2 | **50-1-49** | 61.45 | 25 / 69 | 25.8% | 20.6% | 0.41 | 0.01 | **3** | 3.43 / 1.80 |
| counter P1 vs enh P2 (50) | 21-0-29 | 52.34 | 14 / 31 | 26.5% | 20.4% | 0.56 | 0.02 | 3 | 3.22 / 1.77 |
| enh P1 vs counter P2 (50) | **33-1-16** | 55.96 | 17 / 30 | 23.4% | 19.1% | 0.74 | 0.24 | 0 | 3.47 / 1.74 |

† 39 of 40 SWAPs in that cell are one 200-turn draw (game 30). Without it,
swaps/g is 0.01.

**Exactplay vs enhanced, combined: 108-2-90** in 200 games (~54%). Scale
200 is a real but modest edge over scale 100 when geometry is shared
(`eval_center_occ=180` both sides).

**Counterplay vs enhanced, combined: 37-1-62** in 100 games. Enhanced
already beats counterplay from both seats. The 141-1-58 exactplay vs
counterplay crush is therefore **mostly leaving `eval_center_occ=70`**,
not doubling the braid term.

Pick composition on the isolation cells stays #83-like (1-ring bench
~9.6%, single-braid ~4.3%, bag ~75% 1-ring). Same-deck 27 identical on
every log.

## Hillclimb (existing leaf weights only)

[`tune_eval.py`](tune_eval.py) walks neighbors of the current champion on
the four `SearchConfig` eval knobs. No pick bonuses, extra qsearch,
move-order tactics, priors, or subsearch. Each candidate plays the
champion in **both seats** (`--games 10` × 2 pairings × 2 seats = 40
games, `--budget nodes:12000,iters:250`). Promote if candidate net
(W−L) ≥ 4.

```bash
python3 apa-kan/game/benchmark_results/exactplay/tune_eval.py climb \
  --start exactplay --seed 7 --games 10 \
  --budget nodes:12000,iters:250 --jobs 4 --rounds 3 --promote-net 4 \
  --work apa-kan/game/benchmark_results/exactplay/tune
```

Step `s` uses bench `--seed = 7 + 17*(s+1)`. Trajectory:
[`tune/trajectory.jsonl`](tune/trajectory.jsonl). Champion:
[`tune/champion.toml`](tune/champion.toml) (same weights as
[`exactplay.toml`](../../bots/exactplay.toml)).

| Step | Move | Seed | Cand W-D-L | Net | Promote? |
|---|---|---|---|---|---|
| 0 | `eval_margin_scale` 200→150 | 24 | 21-0-19 | +2 | no |
| 1 | `eval_margin_scale` 200→250 | 41 | 20-0-20 | 0 | no |
| 2 | `eval_center_occ` 180→120 | 58 | 21-0-19 | +2 | no |
| 3 | `eval_center_occ` 180→240 | 75 | 18-0-22 | −4 | no |
| 4 | `eval_inner_ring` 55→40 | 92 | 21-0-19 | +2 | no |
| 5 | `eval_inner_ring` 55→70 | 109 | 14-0-26 | **−12** | no |
| 6 | `eval_center_pressure` 0→50 | 126 | 20-0-20 | 0 | no |

Round 0 local peak: **200 / 180 / 55 / 0**. Inner 70 (counterplay’s inner
weight) and center 240 are the only clearly worse neighbors on this
sample. 150 / occ 120 / inner 40 are +2, below the promote threshold.

Because the champion did not move, confirmation is the isolation 100-game
cells vs enhanced (**108-2-90**) plus the committed 100-game H2H vs
counterplay (**141-1-58**), same budget `nodes:33000,iters:650`, `--seed 1`.
`tune_eval.py confirm` re-runs those cells under `tune/` if you want a
fresh seed-matched package.

## Qualitative log review (winning bot)

Citations are `log` / `pairing` / `game` / `turns`. Inner-ring node ids are
`{9,1,12,3,16,6,18,21}`; center is 0. Every first take in the isolation
cells is a MOVE.

### Sit-and-win is the actual win path — a healthy occupant plus inner 5

[`h2h_exact_vs_enh_p1.jsonl.gz`](h2h_exact_vs_enh_p1.jsonl.gz) / mcts vs
minimax / game 47 / 21 turns. Exactplay is P1. P1 MOVEs 2→0 at t5 with a
**3-braid** and never leaves. Final control 5–4: P1 on `{0,1,12,18,21}`,
P2 on `{3,6,9,16}`. Control-5 hold. No SWAP, no CLAIM, no center pop.

Same line as P2:
[`h2h_exact_vs_enh_p2.jsonl.gz`](h2h_exact_vs_enh_p2.jsonl.gz) / mcts vs
minimax / game 25 / 20 turns. Exactplay minimax MOVEs 2→0 at t8 with a
**2-normal**, never leaves. Final P2 `{0,1,9,16,21}` vs P1 `{12,18}`.

Vs counterplay the same sit beats a bot that downweights the origin:
[`h2h_exact_as_p2.jsonl.gz`](h2h_exact_as_p2.jsonl.gz) / mcts vs minimax /
game 1 / 28 turns. Exactplay P2 MOVEs 23→0 at t8 with a 2-normal, never
leaves, control-5.

Combined exact vs enhanced sit-and-win is 42/186 takes (22.6%), matching
self-play 22.6% and #83 22.9%. Scale 200 does not invent a new line; it
slightly prefers keeping a scoring-healthy occupant.

### Lockout after an origin fight

[`h2h_exact_vs_enh_p1.jsonl.gz`](h2h_exact_vs_enh_p1.jsonl.gz) / mcts vs
minimax / game 6 / 61 turns. Enhanced P2 MOVEs onto 0 at t8. Exactplay
pops that occupant from inner 6 at t13 (mutual with self on 6), pops
again at t50 from 16, retakes 0 at t52, self-pops at t60 while capturing
P2 on 5, then lockout-wins. Empty center at the end. This is the
cascade/lockout path: inner MOVE that mismatches the occupant, not a
scripted SWAP steal.

Vs counterplay the blowout seat is the short cascade:
[`h2h_exact_as_p1.jsonl.gz`](h2h_exact_as_p1.jsonl.gz) / minimax vs mcts /
game 1 / 29 turns. P1 3-ring onto 0 at t7, vacates t17, P2 retakes t26,
P1 lockout-wins. Pairing 44-0-6.

### SWAP is not a win path

Isolation exact-P1 cell: **40 SWAPs in 100 games, 39 of them in one
200-turn draw.**
[`h2h_exact_vs_enh_p1.jsonl.gz`](h2h_exact_vs_enh_p1.jsonl.gz) / mcts vs
minimax / game 30 / 200 turns. Enhanced P2 (and later both) rotate a
1-normal around the board including 21→0 / 1→0 / 3→0 at budget-starved
plies; draw on turn-limit. The other SWAP is game 40, 20→21, never
center. Dislodging SWAPs are not how exactplay wins.

### CLAIM is still a sideshow

Three CLAIMs in 200 isolation games vs enhanced, all in the exact-as-P2
cell. [`h2h_exact_vs_enh_p2.jsonl.gz`](h2h_exact_vs_enh_p2.jsonl.gz) /
mcts vs minimax / game 7 / 42 turns. Enhanced P1 CLAIMs its **own**
overwhelmed 2-normal on node 6 at t37. Exactplay P2 already sits a 3-zag
on 0 and wins control. Game 21 CLAIMs an opponent 3-braid on inner 9 at
t99 during a long walk; game 24 CLAIMs a 2-zag on 20. None of the three
are the winning action.

### Single-braid origin hits are losses here, not wins

Two single-braid center pops in the exact-P1 cell, **both enhanced P2
beating exactplay**:
[`h2h_exact_vs_enh_p1.jsonl.gz`](h2h_exact_vs_enh_p1.jsonl.gz) / minimax
vs mcts / game 31 / 61 (P2 MOVE 11→6 1-braid pops P1 on 0, lockout) and
game 49 / 54 (P2 MOVE 20→1 1-braid, lockout). Exactplay did not find a
single-braid snipe as a win path at this budget. Self-play had 0 such
pops; vs counterplay 0.

### Never-left / pick / depth (winning bot, isolation + confirm)

| Cell | Games | Never-left | Sit-and-win | Center rem/g | Swaps/g | Lockouts | CLAIM | 1-ring bench | mm / mcts |
|---|---|---|---|---|---|---|---|---|---|
| exact vs enh (200) | 200 | 26.3% | 22.6% | 0.49 | 0.21† | 56 | 3 | 9.6% | 3.46 / 1.80 |
| exact vs counter (200) | 200 | ~30% | ~19.5% | 0.58 | 0.07 | 59 | 0 | 9.5% | ~3.45 / 1.75 |
| exact self | 100 | 24.7% | 22.6% | 0.47 | 0.02 | 24 | 0 | 9.5% | 3.48 / 1.78 |
| #83 counterplay | 100 | 26.0% | 22.9% | 0.82 | 0.16 | 28 | ~0 | 9.6% | 3.53 / 1.77 |

Playstyle matches `enhanced` / #83, not a scripted contester. The extra
wins vs enhanced are the same sit-and-win and lockout-after-fight lines
with a leaf that values fulfilled-braid margin more.

## Deep-search probe (does sitting win only because search is shallow?)

Liam’s question: sit-and-win at light budget (MCTS ~1.8 ply, minimax ~3.5)
might be a horizon artifact. This probe raises **in-game** work only.
Search-pick stays on `nodes:33000,iters:650` so the 11-tile benches match
the committed 100-game self-play cell. No new tactics, priors, or
ordering. `--p1-budget` / `--p2-budget` overlay the seat’s `SearchLimit`
after pick.

```bash
APA_KAN_BENCH=target/release/apa-kan-bench \
  bash apa-kan/game/benchmark_results/exactplay/run_deep.sh all
```

`--games 5` × 2 pairings = **10 games per cell**, 4 cells = **40 games**.
4x = `nodes:132000,iters:2600`. 8x = `nodes:264000,iters:5200`. Same
live #83 defaults, SUM, `--seed 1`, `--same-deck on`, `--no-default-features`,
release. 4x self txt omits `# in_game_budget` because both seats match
each other (later benches print it whenever either seat differs from
pick); pairing rows show `avg_iters=2600` / `5200`.

### Metrics vs light baseline

Sit-and-win is never-left *and* the first-taker’s side won, among games
with a surviving take. Contested = holders>1 or a center opponent-pop or
a vacate. The seed-1 first-10 slice of [`h2h_self`](h2h_self.jsonl.gz) is
the same 10 deals as the deep cells (5 per pairing).

| Cell | Games | P1 W-D-L | Turns | Never-left | Sit-and-win | Contested | Vacates/g | Center rem/g | Swaps/g | Lockouts | CLAIM | SB pops | mm / mcts |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| exactplay self (light) | 100 | 48-2-50 | 58.41 | **24.7%** | **22.6%** | 75.3% | 1.33 | 0.47 | 0.02 | 24 | 0 | 0 | 3.48 / 1.78 |
| light self, seed-1 first 10 | 10 | 3-0-7 | 54.00 | **33%** (3/9) | **33%** (3/9) | 67% | 1.10 | 0.60 | 0.00 | 2 | 0 | 0 | (subset) |
| **4x self** | 10 | 5-1-4 | **101.00** | **0%** | **0%** | **100%** | **3.70** | 0.20 | 0.10 | 2 | 0 | 0 | **4.84 / 2.24** |
| **8x self** | 10 | 4-1-5 | **112.10** | **10%** | **10%** | **90%** | **4.30** | 0.70 | 0.00 | 2 | 0 | 0 | **5.39 / 2.54** |
| 4x vs light (deep P1) | 10 | **7-0-3** | 63.00 | 10% | 10% | 90% | 1.90 | 0.40 | 0.10 | 1 | 0 | 0 | 4.03 / 1.99 |
| 4x vs light (deep P2) | 10 | 2-0-8 | 86.60 | 10% | 0% | 90% | 3.20 | 0.50 | 0.00 | 3 | 0 | 0 | 4.28 / 2.07 |

**Deep 4x vs light exactplay, combined: 15-0-5** in 20 games. Extra ply
is a real strength gain, not just a style shift.

Never-left / sit-and-win collapse in self-play (24.7% / 22.6% → 0% at
4x; 10% / 10% at 8x). Vacates/game 1.33 → 3.70 / 4.30. Center
*pops* do not explode (0.47 → 0.20 at 4x, 0.70 at 8x): the extra ply
mostly **walks off and retakes**, plus some inner MOVEs that mismatch
the occupant. CLAIM stays 0. Single-braid center pops stay 0. SWAP
stays ~0 (one own-rotate at 4x). Lockouts stay ~20%. Games get much
longer (58 → 101 / 112 turns); zero-budget share 27% → 41% / 36%.
1-ring benches stay ~9.1% (same pick budget).

MCTS is still shallow: 1.78 → 2.24 → 2.54 ply. Minimax 3.48 → 4.84 →
5.39. 8x is about **+1.4 ID ply / +0.8 MCTS ply**, not a different
engine.

### Qualitative (same deals, light vs deep)

Citations are `log` / `pairing` / `game` / `turns`. Inner-ring node ids
are `{9,1,12,3,16,6,18,21}`; center is 0. First take is MOVE in every
deep game. CLAIM is 0.

#### Matched seed-1 game 1: light sits, deep fights

Light [`h2h_self.jsonl.gz`](h2h_self.jsonl.gz) / mcts vs minimax / game 1
/ 32. P2 MOVEs onto 0 at t18 with a 2-normal and **never leaves**.
Control-5. This is the README’s sit-and-win example.

Same deal at 4x:
[`h2h_deep4x_self.jsonl.gz`](h2h_deep4x_self.jsonl.gz) / mcts vs minimax /
game 1 / 144. P2 MOVEs 23→0 at t16 (2-normal), **sits until t72**, then
vacates 0→8. P1’s 3-ring onto 0 at t73 self-pops. P2 retakes t78, walks
off t84. P1’s 3-zag onto 0 at t91 self-pops. More 0↔inner walking. P1
wins control. Not a pop war (0 opponent center caps); a leave/retake
fight.

Same deal at 8x:
[`h2h_deep8x_self.jsonl.gz`](h2h_deep8x_self.jsonl.gz) / mcts vs minimax /
game 1 / 145. P2 takes t16, **leaves at t28** (not t72), walks 0↔inner
on a 2-braid, ends with empty-center self-pops. P2 wins control. Deeper
search leaves *earlier*.

#### Deep self-play wins by leave / inner-pop / retake, not by sitting

[`h2h_deep4x_self.jsonl.gz`](h2h_deep4x_self.jsonl.gz) / mcts vs minimax /
game 3 / 84. P2 takes 6→0 at t10, **vacates t14** onto 18 (inner cap).
Then 0↔21 walks at budget 0. P1 finally takes 6→0 at t63. Lockout, P1.
Holders=6, vacates=5, 0 center pops.

[`h2h_deep4x_self.jsonl.gz`](h2h_deep4x_self.jsonl.gz) / minimax vs mcts /
game 4 / 60. P1 takes t5, vacates t35, retakes t45. P2’s inner MOVE
23→21 at t46 **pops the occupant** (mutual). P1’s t59 onto 0 self-pops.
P2 retakes t60 and lockout-wins. This is the inner-mismatch fight, not
SWAP.

[`h2h_deep8x_self.jsonl.gz`](h2h_deep8x_self.jsonl.gz) / mcts vs minimax /
game 3 / 57. P2 takes t16. P1 pops from inner 6 at t19 (mutual). P2
retakes t22, walks. P1 retakes t47. P2 pops from 14→12 at t56. P1
retakes t57 and lockout-wins. Four occupancy changes, two opponent pops.

#### Sitting is not dead — it is rarer, and sometimes still correct

[`h2h_deep8x_self.jsonl.gz`](h2h_deep8x_self.jsonl.gz) / mcts vs minimax /
game 2 / 39. P2 MOVEs 11→0 at t6 with a 2-braid and **never leaves**.
Control-5. 1/10 8x games is sit-and-win.

[`h2h_deep4x_vs_light_p1.jsonl.gz`](h2h_deep4x_vs_light_p1.jsonl.gz) /
mcts vs minimax / game 2 / 53. **Deep** P1 MOVEs 20→0 at t11 with a
2-braid and never leaves. The leftover sit-and-win in the vs-light
cells is the *deep* bot sitting on a healthy occupant, not the light
bot.

Leaving can also lose:
[`h2h_deep4x_vs_light_p1.jsonl.gz`](h2h_deep4x_vs_light_p1.jsonl.gz) /
mcts vs minimax / game 1 / 44. Deep P1 takes t7, vacates t11, retakes
t15. Light P2 pops from 17→18 at t16, takes t20, wins control. Extra
ply is not a scripted “always contest.”

#### Deep vs light: depth punches the sitter and still lockout-fights

[`h2h_deep4x_vs_light_p2.jsonl.gz`](h2h_deep4x_vs_light_p2.jsonl.gz) /
mcts vs minimax / game 4 / 32. Deep P2 takes 3→0 at t18, brief leave
t20 (inner cap on 23), retakes t22, lockout. 32 turns.

[`h2h_deep4x_vs_light_p2.jsonl.gz`](h2h_deep4x_vs_light_p2.jsonl.gz) /
minimax vs mcts / game 5 / 57. Light P1 takes t5. Deep P2 pops from
inner 9 at t10, retakes t12. Light retakes t51. Lockout, **light**
wins this one — one of the 5/20 light wins.

### Distillation retune (implemented; no promote)

`--compare-roots` replays [`h2h_deep4x_self.jsonl.gz`](h2h_deep4x_self.jsonl.gz)
and re-searches each ply at light. [`distill_order.py`](distill_order.py)
scores the four existing `cheap_order` integers on **disagreements
only**, with extra weight on center-touching moves. Train = games 1–3
of each pairing; hold = games 4–5. No new features.

1011 roots, 664 disagreements (331 train / 333 hold). Grid: 120
neighbors of `vacate ∈ {0,400,800,1200,2000}`, `vacate_ow ∈ {0,1200}`,
`threat ∈ {0,2000,3500,5000}`, `claim ∈ {0,2500,5000}`.

| | vacate | vacate_ow | threat | claim | train wMRR | hold wMRR |
|---|---|---|---|---|---|---|
| current / champion | **1200** | **1200** | **0** | **0** | **0.217** | **0.151** |
| vacate=0 (best vacate-rank neighbor) | 0 | 1200 | 0 | 0 | 0.201 | — |

Before = after. Mean rank of deep’s chosen move under current
`cheap_order` is ~16 (train) / ~19 (hold). Top-1 is 8% / 5%. The
vacate tax only applies to 11 train disagreements; zeroing it improves
those ranks 16.1→6.8 but hurts overall wMRR. Threat disagreements are
already rank 2.8 via capture bonuses (`threat=3500` is a no-op). Deep
never chose CLAIM, so `order_claim_center` cannot fit. Extra ply is not
a one-knob ordering fix. **No TOML promoted; skipped 4x-vs-candidate
confirm.** Champion remains [`exactplay.toml`](../../bots/exactplay.toml).

## Deep wide search (not a random-player baseline)

Liam’s correction: fan the **deep** tree out, do not add weak/epsilon
policies. Overlay on `exactplay` at the same 4x (and a small 8x) in-game
budget, both seats, two seeds:

`puct=8,root-noise=0.25,dirichlet-alpha=0.3,ordering=off,lmr=off,pvs=off,untried-random=1`

Heuristic playouts stay **on**. New knobs `mcts_root_noise` / Dirichlet
`α` default to 0 / 0.3 so live `exactplay` is bit-identical. Minimax
shuffles the root when `ordering=off`. Reproduce:
[`run_explore.sh`](run_explore.sh). Pure-random-player baseline skipped
(not cheap at this budget, and not the question). These cells predate
ratchet + 2-2-1-1; re-runs inherit the live lock unless those flags are
pinned off.

Seed-1 4x cells share the 27-tile deals with [`h2h_deep4x_self`](h2h_deep4x_self.jsonl.gz).
Wide as P1 also overlays search-pick, so first ADD usually diverges;
wide as P2 keeps P1’s first ADD (standard).

| Cell | Games | Wide W-D-L | Turns | Never-left | Sit-and-win | Vacates/g | CLAIM | SWAP | SB pops | mm / mcts | entropy / eff_branch / top1 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 4x self (standard) | 10 | — (5-1-4 P1) | 101 | **0%** | **0%** | 3.70 | 0 | 1 | 0 | 4.84 / 2.24 | 2.35 / 14.6 / 0.35 |
| **wide 4x as P1** (seed 1) | 10 | **5-0-5** | 59 | 30% | **20%** | 1.10 | 0 | 3 | 0 | 3.71 / 2.05 | **2.69 / 18.5 / 0.27** |
| **wide 4x as P2** (seed 1) | 10 | **2-1-7** | 75 | 33% | **33%** | 2.20 | 0 | 0 | 0 | 4.69 / 2.04 | **2.80 / 20.4 / 0.23** |
| wide 4x as P1 (seed 11) | 6 | 3-0-3 | 88 | 50% | 33% | 1.33 | 0 | 0 | 0 | 4.25 / 2.00 | 2.85 / 21.6 / 0.25 |
| wide 4x as P2 (seed 11) | 6 | 2-1-3 | 97 | 20% | 20% | 1.83 | 0 | 0 | 0 | 4.30 / 1.94 | 3.05 / 26.8 / 0.21 |
| wide 8x as P1 | 4 | 3-0-1 | 139 | 0% | 0% | 5.50 | 0 | 4† | 0 | 5.48 / 2.18 | 2.74 / 18.7 / 0.23 |
| wide 8x as P2 | 4 | 1-0-3 | 63 | 50% | 50% | 0.25 | 0 | 0 | 0 | 4.60 / 2.10 | 2.73 / 20.6 / 0.29 |

† All four 8x SWAPs are edge 1-normal walks, none on center.
**Wide vs standard 4x, combined (both seats, both seeds): 12-2-18.**
**Wide vs standard 8x, combined: 4-0-4.** Fan-out is real (entropy and
effective branching up, top-1 share down) but at a fixed node budget it
**costs depth** (wide-as-P1 mm 3.71 vs standard 4.84) and **sits more**,
like light search. It does not uncover a hidden origin tactic.

### Strategies standard 4x never plays — with citations

Citations are `log` / `pairing` / `game` / `turns`. Inner-ring
`{9,1,12,3,16,6,18,21}`; center is 0.

**CLAIM: still 0.** 32 games at 4x wide + 8 at 8x wide. Branching-factor
sample also has 0 legal CLAIMs. Not hidden by pruning; rarely legal,
never chosen.

**Single-braid center pops: still 0.** Same as 4x/8x self.

**SWAP is not a new win path.** The only SWAPs that are *absent* from
the 4x-self corpus are in
[`explore/h2h_wide4_p1.jsonl.gz`](explore/h2h_wide4_p1.jsonl.gz) /
minimax vs mcts / game 2 / 151. Wide P1 (minimax) takes 17→0 at t3 with
a 1-normal, **SWAP 0→12 at t67 budget 0**, then loses control to
standard P2. Same deal at 4x self: P1 SWAP **0→16** at t39 with the
*same* tile (id 16) and **P1 wins**. Wide found a worse vacate-via-SWAP,
not a steal. The other two SWAPs in that game (t30 7→8, t140 5→4) are
**standard P2** rotating a 1-normal on the edge. 8x wide P1’s four
SWAPs are the same edge walk (23→21 / 13→10 / 14→15 / 15→17), none
center.

**Sit-and-win comes back when the tree is wide-and-shallow.**
[`explore/h2h_wide4_p1.jsonl.gz`](explore/h2h_wide4_p1.jsonl.gz) /
minimax vs mcts / game 3 / 27. Wide P1 MOVEs 20→0 at t13 and never
leaves; control-5. Same 27 at 4x self went 125 turns, P2 won — but
first ADD already differed (wide 5 vs standard 2), so this is a
different opening, not a hidden PV. Fairer (wide as P2, first ADD
matches): the sit-and-wins are mostly **standard P1** sitting on wide
P2, e.g.
[`explore/h2h_wide4_p2.jsonl.gz`](explore/h2h_wide4_p2.jsonl.gz) /
minimax vs mcts / game 4 / 35 (standard P1 sits t15, wins; 4x self P2
won that deal at 60) and
[`explore/h2h_wide8_p2.jsonl.gz`](explore/h2h_wide8_p2.jsonl.gz) /
mcts vs minimax / game 2 / 25 (standard P1 sits t11, wins). One wide
sitter that wins:
[`explore/h2h_wide4_p2.jsonl.gz`](explore/h2h_wide4_p2.jsonl.gz) /
minimax vs mcts / game 5 / 42, wide P2 sits t14; 4x self also P2-won
that deal (168 turns). Sitting is not a *new* line.

**Stalling / 200-turn draws.**
[`explore/h2h_wide4_p2.jsonl.gz`](explore/h2h_wide4_p2.jsonl.gz) /
minimax vs mcts / game 3 / 200, draw on turn-limit (4x self P2 won at
125).
[`explore/h2h_wide4_p2_s11.jsonl.gz`](explore/h2h_wide4_p2_s11.jsonl.gz)
/ minimax vs mcts / game 1 / 200. Same degenerate ZB walking as 4x/8x
self (1/10 each), not a winning stall tactic.

**Unusual picks / edge play.** 1-ring bench stays 9.1% on seed-1 (same
pick budget / same 27). Seed 11 happened to draft 1.5% 1-ring — deal
noise, not a search policy. Inner→edge MOVEs are common in both
standard and wide logs; they are the leave/retake walk, not a new
edge-rush.

**Outcome flips on matched seed-1 deals (wide as P2, first ADD
matches):** 3/10. Two are standard punching a wide sitter or avoiding
a 200-turn draw; none is wide inventing CLAIM/SWAP/SB and winning.

## Fairness + degenerate-tactic audit

Wilson 95% intervals. `audit_fairness.py` counts `budget==0` literally
(not `or 1`). Same-deck 27 is identical across pairings on every cell.

| Cell | n | P1 W-D-L | P1% [95%] | t200 | Lockout | CLAIM | SWAP | SB pops | ZB |
|---|---|---|---|---|---|---|---|---|---|
| exactplay self (light) | 100 | 48-2-50 | **48.0 [38.5, 57.7]** | 1 | 24% | 0 | 2 | 0 | 29.4% |
| exact P1 vs counter P2 | 100 | 71-0-29 | 71.0 [61.5, 79.0] | 0 | 31% | 0 | 10 | 0 | 29.3% |
| counter P1 vs exact P2 | 100 | 29-1-70 | 29.0 [21.0, 38.5] | 1 | 28% | 0 | 3 | 0 | 30.5% |
| exact P1 vs enhanced P2 | 100 | 59-1-40 | 59.0 [49.2, 68.1] | 1 | 31% | 0 | 40† | 2 | 29.0% |
| enhanced P1 vs exact P2 | 100 | 50-1-49 | 50.0 [40.4, 59.6] | 1 | 25% | **3** | 1 | 0 | 28.9% |
| 4x self | 10 | 5-1-4 | 50 [24, 76] | 1 | 20% | 0 | 1 | 0 | 42.5% |
| 4x vs light (deep P1) | 10 | 7-0-3 | 70 [40, 89] | 0 | 10% | 0 | 1 | 0 | 32.4% |
| 4x vs light (deep P2) | 10 | 2-0-8 | 20 [6, 51] | 0 | 30% | 0 | 0 | 0 | 42.6% |
| 8x self | 10 | 4-1-5 | 40 [17, 69] | 1 | 20% | 0 | 0 | 0 | 37.8% |
| wide 4x as P1 | 10 | 5-0-5 | 50 [24, 76] | 0 | 20% | 0 | 3 | 0 | 29.9% |
| wide 4x as P2 | 10 | 7-1-2 | 70 [40, 89] | 1 | 20% | 0 | 0 | 0 | 46.3% |

† 39/40 isolation SWAPs are one 200-turn draw (exact vs enh P1, mcts vs
minimax, game 30).

**Seat balance.** Light self-play is even (48%, CI covers 50%). The
71/29 split vs counterplay is the **eval gap** (geometry 180 vs 70),
not a P1-seat bug: it flips when exactplay sits in seat 2. Deep vs
light 15-0-5 is a **depth gap**, same pattern both seats. Wide vs
standard 12-2-18 is a **breadth-vs-depth** gap, also both seats.
n=10 CIs are huge; do not over-read 4x/8x P1%.

**Engine asymmetry.** Exactplay minimax-as-P1 vs counterplay MCTS is
44-0-6 (88% [76, 94]); exactplay MCTS-as-P1 vs counterplay minimax is
27-0-23 (54% [40, 67]). Minimax gets more from the leaf scale than
MCTS does. Light self-play pairing split is 23-2-25 vs 25-0-25 — no
self-play engine crush.

**Draws / 200-turn / lockout.** Turn-limit is 1/100 light, 1/10 at 4x
and 8x, 1/10 wide-as-P2: extra ply *lengthens* games (58 → 101 / 112)
via 0↔inner walking at budget 0 (ZB 29% → 38–43%), then some hit the
cap. Lockouts 24% light, ~20% deep — not rising. Control remains the
main end (~70%). Low-value and elimination stay rare.

**Same-deck first-move luck.** Every log: same-deck 27 identical across
the two pairings. First action is always ADD (never MOVE/SWAP/CLAIM).
Opening 1-ring bench is ~9.5% whenever pick is light (seed 11’s 1.5%
is that deal’s 27, not a policy).

**Dead vs unseen.** CLAIM is *dead* as a chosen action (0 in 300
H2H + 40 deep + 32 wide-4x + 8 wide-8x; 3 in 200 isolation, none a
win path — own-tile or inner). SWAP is *seen* but degenerate
(budget-0 walks, one own-rotate). Single-braid origin snipes are
*unseen* in exactplay self/deep/wide; the only two in isolation are
enhanced P2 beating exactplay. Overwhelm is still MOVE. Nothing in
the wide sweep promotes those mechanics from unseen to useful.

## Open design questions

- **Scale vs geometry is now measured.** Exactplay vs enhanced 108-2-90;
  counterplay vs enhanced 37-1-62. The vs-counterplay crush is mostly
  “don’t downweight the origin.” Is a ~54% scale edge enough to flip the
  playtest default off `enhanced`?
- **+2 neighbors were not promoted.** Scale 150, occ 120, and inner 40
  each went 21-0-19 on 40 games. That is noise under promote-net 4, but
  it is also the direction *away* from counterplay’s 70/70/100. A 100-game
  probe of those three would say whether 200/180/55 is a plateau or a
  coarse grid artifact. SPSA/CMA on continuous weights was not run.
- **Hardcoded leaf terms were not searched.** Material, contact, spawn,
  bench, budget, presence, mobility, and claimable in `eval.rs` are not
  `SearchConfig` knobs. `order_vacate_*` is move ordering, not eval, so
  the hillclimb left it alone; the later disagreement fit also did not
  move it.
- **Self-play at light still does not contest more.** Never-left /
  sit-and-win / SWAP / pick match #83. Extra ply *does* contest: 4x
  self sit-and-win 0/10, 8x 1/10, vacates 1.33 → 3.7–4.3. Sitting is
  mostly a horizon artifact, not *only* one (8x game 2 still sits).
  Programming that fight by hand is still the circular argument.
- **Distillation did not move `cheap_order`.** 66% of 4x plies disagree
  with light, but the four existing integers cannot rank deep’s leave
  (mean rank ~16). Vacate=0 helps 11 examples and hurts the rest.
  Fitting would need new features (recapture-after-vacate) or a live
  budget raise — both out of scope here.
- **Deep-and-wide at the same node budget is weaker, not more tactical.**
  Combined 12-2-18 vs standard 4x. CLAIM/SB still 0. SWAP still
  degenerate. Fan-out (entropy 2.35→2.7–3.0) buys sit-and-win again
  because mm depth drops 4.84→3.71. To actually search *hidden* lines
  we would need wide *and* more nodes, not a PUCT/Dirichlet overlay
  alone.
- **CLAIM still dead** (3/200 vs enhanced, 0 everywhere else including
  wide). Overwhelm is still MOVE. The three CLAIMs are own-tile or
  inner, not origin steals. Branching-factor samples often have 0
  legal CLAIMs.
- **MCTS ~1.8 ply at light, ~2.5 at 8x, ~2.0 at 4x wide.** Extra nodes
  buy less than a ply of MCTS depth; extra exploration buys even less.
  Distill ordering failed; raise the live budget?
- **n=10 per deep cell.** Sit-and-win 0/10 vs 1/10 is noisy. Wide n=32
  at 4x is enough to say CLAIM/SB stay dead, not enough to pin a 40%
  wide-vs-standard rate. 8x vs light was skipped for runtime.
- **Center pops did not rise with 4x** (0.47 → 0.20) **or with wide**
  (0.20–0.50). Contest is vacate/retake, not dislodge. Fitting
  `order_center_threat` from these logs does nothing (already rank 2.8);
  fitting `order_vacate_center` also failed.
- **Search pick still dumps 1-rings** (9.5% on the 11 vs ~49% generated).
  Existing heuristic, not a new bias.
- **One SWAP-walk draw dominates swap stats.** Quote swaps/g from
  isolation with that caveat, or exclude turn-limit games.
- **Lockouts 24%** in self-play (was 28% on #83, 39% with tactics).
  Control wins rose 66 → 71. Isolation vs enhanced is 56 lockouts / 200.
  Acceptable?
- **`tile_score` includes weight** in the match total, so `eval_margin_scale`
  amplifies weight-as-score as well as braid hits. A braid-only scale
  (leave non-braid match at ×8) was not a knob.
