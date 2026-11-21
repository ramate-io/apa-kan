# Exactplay bots: eval-only leaf margin for braid-only-exact

Stacked on PR #83 (live playtest `RuleSet`: 4:3:1 Additive, `braid-only-exact`,
single braids on, normal-only SWAP, center↔inner double-count, color-drift
160). That PR’s confirmation cell
[`h2h_defaults`](../single_braid_exact_swap/h2h_defaults.txt) is the **old
bot** baseline: `--search-profile counterplay`, `--pick search`.

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
enhanced), and [`tune_eval.py`](tune_eval.py) (coordinate descent). Survey:
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
  it stayed off-limits.
- **Self-play still does not contest more.** Never-left / sit-and-win /
  SWAP / pick match #83. If the playtest goal is “fight for origin,” a
  scoring-faithful leaf is not sufficient at this budget — and
  programming that fight is the circular argument Liam flagged.
- **CLAIM still dead** (3/200 vs enhanced, 0/300 vs counterplay + self).
  Overwhelm is still resolved by MOVE. The three CLAIMs are own-tile or
  inner, not origin steals.
- **MCTS ~1.8 ply.** Same as #83. No priors.
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
