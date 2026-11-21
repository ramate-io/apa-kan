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

Reproduce with [`run_h2h.sh`](run_h2h.sh). Survey: [`survey.py`](survey.py)
(first center *take* counts only if the landing piece is still there after
that action’s captures resolve).

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

200 is a scoring-faithful guess, not a self-play hillclimb: braid-only match
totals are a subset of mixed-channel matches, so the ×8 term shrank relative
to material. Isolation vs `enhanced` (scale 100) is an open question below.

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

## Qualitative log review

Citations are `log` / `pairing` / `game` / `turns`. Inner-ring node ids are
`{9,1,12,3,16,6,18,21}`; center is 0. Every first take in these 300 games is
a MOVE. CLAIM is 0.

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

## Open design questions

- **Is the H2H win the margin scale, or leaving counterplay’s geometry?**
  `exactplay` is `enhanced` + `eval_margin_scale=200` (`eval_center_occ=180`).
  `counterplay` is `eval_center_occ=70` plus pressure/claim-order knobs. We
  did not run exactplay vs enhanced (scale 200 vs 100) or enhanced vs
  counterplay on this package. The 141-1-58 H2H could be “don’t downweight
  the origin” rather than “2× the braid term.” A 100-game `enhanced` vs
  `exactplay` cell would isolate the scale.
- **200 was not hillclimbed.** 150 vs 250 vs 300 was not probed at 100
  games. `tile_score` also puts `weight` in the match total, so the scale
  amplifies weight-as-score as well as braid hits.
- **Self-play did not contest more.** Never-left / sit-and-win / SWAP / pick
  composition match #83. If the playtest goal is “bots that fight for
  origin,” a scoring-faithful leaf is not sufficient at this budget — and
  programming that fight is the circular argument Liam flagged.
- **CLAIM still dead** (0/300 games, 0 heals that used CLAIM; 3 heals in
  self-play via other actions). Overwhelm is still resolved by MOVE.
- **MCTS ~1.8 ply.** Same as #83. No priors this time, so we did not spend
  the budget on a scripted prior either.
- **Search pick still dumps 1-rings** (9.5% on the 11 vs ~49% generated).
  That is the existing heuristic, not a new bias. Search-pick rollouts see
  the scaled leaf; they still do not keep singles.
- **Live bot default.** UI / `cargo run` still uses `enhanced` unless
  `--search-profile exactplay`. Flip playtest default to exactplay, keep
  enhanced, or wait on a scale-vs-enhanced cell?
- **Lockouts 24%** in self-play (was 28% on #83, 39% with tactics). Control
  wins rose 66 → 71. Acceptable?
