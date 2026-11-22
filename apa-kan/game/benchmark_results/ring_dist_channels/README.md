# Ring distribution, channels, same-deck 27, and ColorPass.drift

> **Archived as a channel-mode experiment.** Live rings are 4:3:1, same-deck
> 27, drift 160, additive. `--channel-mode` was removed. See
> [`../archive/`](../archive/).


Round 1 (the original `h2h_a_*` / `h2h_b_*` logs) is the 7:5:4 braid:bead
package. Do not overwrite those files. Round 2 is the same-deck 27 follow-up:
beads in the normal channel, ChannelMode scoring-only, braid ~10% of **tiles**,
4:3:2 / 4:3:1, and RFC ColorPass.drift. Reproduce round 2 with
[`run_h2h_same_deck.sh`](run_h2h_same_deck.sh) and
[`run_h2h_color_drift.sh`](run_h2h_color_drift.sh).

---

# Round 1 — 7:5:4, ~10% braid rings, independent channels, counterplay search

Live generation on `main` used a sequential 7:1 keep-last vs draw-any gate
(`RING_SAME_WEIGHT=7`, `RING_ANY_WEIGHT=1`). That made singles ~83% of mixed
tiles and braid rings ~0.6%. This package measures explicit **7:5:4**
singles:doubles:triples, IndependentMax heavy 2:1 (~10% braid rings),
independent-channels on vs off, and default `enhanced` search vs a new
`counterplay` personality.

Do not overwrite committed `h2h_*` logs; quote numbers from those files.

## Package

Release `apa-kan-bench`, `--no-default-features`, `--budget nodes:33000,iters:650`,
`--games 10`, `--jobs 4`, `--no-depth-probe`, `--zags on`, `--zag-mismatch sum`,
`--endgame-rule control-cascade`, `--cascade-turn-gain on`, `--capture-mode local-claim`,
`--no-same-piece-twice off`, `--pick search`,
`--pairings minimax:mcts,mcts:minimax`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`,
`--seed 1`. 10 games × 2 pairings × 5 matchups = **100 games** per config.

| Log | Channels | Search | Games |
|---|---|---|---|
| [`h2h_a_additive_enhanced`](h2h_a_additive_enhanced.txt) | additive / `--independent-channels off` | enhanced | 100 |
| [`h2h_b_imax_enhanced`](h2h_b_imax_enhanced.txt) | independent-max / on | enhanced | 100 |
| [`h2h_a_additive_counterplay`](h2h_a_additive_counterplay.txt) | additive | counterplay | 100 |
| [`h2h_b_imax_counterplay`](h2h_b_imax_counterplay.txt) | independent-max | counterplay | 100 |

Main baseline (old sequential 7:1, independent-max, enhanced) is the
already-committed
[`../control_cascade_turn_gain/h2h_shallow.txt`](../control_cascade_turn_gain/h2h_shallow.txt)
(same budget and matchups).

Reproduce with [`run_h2h_shallow.sh`](run_h2h_shallow.sh). Center-holder
survey: [`survey_center.py`](survey_center.py).

## Generation before / after

`# ring_process` is 20 000 mixed tiles from the config’s `RuleSet` (not
collector stuffing). IndependentMax does not add rings after the count
draw; Additive braid/bead groups promote singles into 2–3.

| Sample | n1 | n2 | n3 | P(1)/P(2)/P(3) | braid piece / ring | bead ring |
|---|---|---|---|---|---|---|
| **main** sequential 7:1, IndependentMax | 16634 | 3259 | 107 | 83.2 / 16.3 / 0.5% | 0.32% / **0.55%** | 11.1% |
| **A** 7:5:4 + Additive stuffing | 6362 | 7486 | 6152 | 31.8 / 37.4 / 30.8% | 14.6% / 15.6% | 14.1% |
| **B** 7:5:4 + IndependentMax 2:1 heavy | 8700 | 6271 | 5029 | **43.5 / 31.4 / 25.1%** | 8.3% / **9.7%** | 10.1% |

B matches 7/16, 5/16, 4/16 (43.75 / 31.25 / 25.00) and the ~10% braid-ring
target. A’s extra doubles/triples are Additive braid/bead 2–3 groups on
top of the same 7:5:4 count draw.

Historical gate (for the record): ring 2 tried `1/8` then kept the last
color on a miss; ring 3 did the same. Color collisions plus that coin are
why P(3) was ~0.5% rather than 1/64.

## Head-to-head (quoted)

| Log | P1 W-D-L | Mean turns | Ends (lock / ctrl / LV / TL / elim) | Opp / self cap/game | Claims /game | Turn-gain awarded | Center opp cap/game |
|---|---|---|---|---|---|---|---|
| main (old gen, imax, enhanced) | 47-3-50 | 77.59 | 20 / 73 / 1 / 3 / 3 | 6.10 / 8.73 | 0.07 | 0.71 | 0.44 |
| A additive enhanced | **64-0-36** | **30.37** | 1 / 99 / 0 / 0 / 0 | 1.60 / 1.50 | 0.02 | 0.08 | 0.09 |
| B imax enhanced | 48-3-49 | 63.50 | 3 / 93 / 1 / 3 / 0 | 3.32 / 4.85 | 0.01 | 0.22 | 0.14 |
| A additive counterplay | 57-1-42 | 35.73 | 3 / 96 / 0 / 1 / 0 | 1.94 / 1.80 | 0.00 | 0.02 | **0.28** |
| B imax counterplay | 53-2-45 | 69.55 | 7 / 90 / 1 / 2 / 0 | 4.03 / 5.25 | 0.00 | 0.36 | **0.34** |

Control-cascade mix (target5 / 4 / 3 / 2, immediate vs hold):

| Log | t5 | t4 | t3 | t2 | immediate | hold |
|---|---|---|---|---|---|---|
| main | 21 | 13 | 20 | 19 | 29 | 44 |
| A enhanced | 91 | 1 | 5 | 2 | 3 | 96 |
| B enhanced | 66 | 11 | 10 | 6 | 15 | 78 |
| A counterplay | 90 | 3 | 2 | 1 | 4 | 92 |
| B counterplay | 63 | 7 | 10 | 10 | 9 | 81 |

Search depth (quoted `# search_depth`): A enhanced mm 3.32 / mcts 1.77;
B enhanced 3.26 / 1.72; A counterplay 3.40 / 1.74; B counterplay 3.56 / 1.70.

Matchup mean turns stay braid-heavy under IndependentMax (B enhanced braid
vs braid **106.65**, B counterplay **78.05**). Additive braid vs braid is
~21 turns and almost all control-5 holds.

## Center-holder survey

From [`survey_center.py`](survey_center.py) on the JSONL (center = node 0).
“Still” = the first piece that occupied the center is on the center at the
end. “Never left” = still, and no other occupant in between.

| Log | First take | Still / never left | First-taker side wins | Still-and-that-side-wins | Vacates | Center opp caps |
|---|---|---|---|---|---|---|
| main | 98/100 @ t10.3 | **20.4% / 16.3%** | 53.1% | 18/20 = 90% | 217 | 44 |
| A enhanced | 99/100 @ t7.3 | **81.8% / 78.8%** | 64.6% | 56/81 = 69% | 41 | 9 |
| B enhanced | 99/100 @ t9.1 | 48.5% / 43.4% | 49.5% | 29/48 = 60% | 158 | 14 |
| A counterplay | 100/100 @ t4.7 | 76.0% / 76.0% | 59.0% | 52/76 = 68% | 23 | 28 |
| B counterplay | 100/100 @ t5.4 | 48.0% / 40.0% | 49.0% | 37/48 = 77% | 145 | 34 |

**Center CLAIM is unused in every new log** (0 center claims; main had 0
center claims and 7 claims elsewhere). Dislodges are same-turn local
captures of destination neighbors, not a follow-up claim.

## Search audit — what the bots pursue vs miss

Default `enhanced` (unchanged as the light-bench personality):

**Pursues**

- Occupying the center: geometry **+180** under `control_win`, MOVE-to-center
  order **+900**, vacating a healthy center **−1200**, plus
  `order_center_bonus` **8000** for any center *or inner-ring* touch.
- Control-set occupancy: **+240** per own node of the 9 (center+inner),
  **+900** once at the cascade target. That is the live win condition.
- Immediate captures: LMR skips captures; qsearch is capture-only.
- Match/mismatch on the landing edge: cheap_order **+8 / +18** for match /
  mismatch, ADD braid-fulfill **+14**, unmatched bead **+20**.
- Not repeating while ahead (−110 per extra repeat).

**Over-values**

- Sitting on the center vs contesting the other eight control nodes
  (geometry 180 vs inner 55). Additive 7:5:4 makes control-5 a 19-turn
  grab: A is **91/99** control wins at target 5, **78.8%** never-left.
- Vacating an *overwhelmed* center as much as vacating a healthy one
  (both −1200 on enhanced). The occupant stays put and hopes.
- Inner-ring and center equally in the +8000 order bonus, then the eval
  throws the game back onto the single center node.

**Ignores / prunes (the counterplay holes)**

- Overwhelm-pressure on a center holder is **gated on `center_hold_win`**.
  Live default is `control_win` with `center_hold_win` off, so
  `eval_center_pressure` is 0. A mismatch web around the occupant does
  not score until a capture actually fires.
- `order_center_threat` and `order_claim_center` are 0, so approaching
  an opponent occupant is not ordered ahead of grabbing a vacant center.
- Multi-turn setups: the hop that only *gets in range* is a non-capture.
  LMR reduces it; qsearch never sees the follow-up. Under local-claim,
  leftover overwhelmed tiles that are **not** dest-neighbors need CLAIM
  next turn — and **no new log claimed the center**.
- Doubles/triples/braids/beads as *blockers that freeze a holder* are
  only a cheap_order ADD hint, not an eval term.

`counterplay` (saveable `--search-profile counterplay` /
`bots/counterplay.toml`) reweights those knobs without changing enhanced
defaults: eval center/inner **70/70**, pressure **100**, claim-center
**5000**, threat **3500**, vacate healthy **800**, vacate overwhelmed
**0**, PUCT **2.0**.

Effect: more same-turn pops of the occupant (A 0.09→0.28, B 0.14→0.34
center opp caps/game) and earlier first takes (t7.3→t4.7, t9.1→t5.4).
It does **not** cut the never-left share on Additive (still ~76%) and
only trims IndependentMax never-left 43%→40%. Claims stay at 0. Games
do **not** look much less like “first piece sits, control-5 holds” on
Additive; IndependentMax was already closer to a coin-flip first-taker
and stays that way, with more fighting on the center.

### Log examples

**Sit and hold (A enhanced, typical).** `h2h_a_additive_enhanced`
mcts vs minimax, bead vs braid game 1: P1 MOVE onto center at t9, zero
vacates/captures/claims, control win at t19. Same shape as many of the
78 never-left games.

**Sit and hold on main, but rarer.**
`control_cascade_turn_gain/h2h_shallow` mcts vs minimax, bead vs bead
game 2: P1 MOVE 11→0 at t7 with a 2-ring 1-bead, still there at t19
control hold. When that pattern happens on main it wins **90%**; it only
happens 16% of the time because singles shuffle (217 vacates, braid vs
braid game 8 minimax vs mcts ran 30 occupants over a 200-turn draw).

**Dislodge by local capture, never CLAIM.** Main game 6 mcts vs minimax
bead vs braid: P2 braid-2 takes center t4; P1 from an inner node pops it
at t7 (`bead_contrib`). P2 retakes t10, later CLAIMs node 17 while
`leftover_overwhelmed=1` — the claim is off-center. Same in every new
log: leftover overwhelmed is almost never the center claim.

**Counterplay actually contests the occupant (B counterplay).**
`h2h_b_imax_counterplay` mcts vs minimax, braid vs bead game 2: P2 takes
center at t6, t12, t20, t24; P1 pops it each time at t9, t15, t23, t33
(tokens/bench). Four center recaptures in one game; P1 still wins
control at t83. This is the structure enhanced under-scores.

**Inner-ring control beating a sitter (B counterplay).** mcts vs minimax
bead vs bead game 9: P2 MOVE onto center at t6 with a 3-ring 1-bead and
never leaves; P1 wins control at t21. The 9-node set can outscore a
parked center piece — enhanced geometry (+180 vs +55) fights that plan;
counterplay levels those two terms.

## Open design questions

- IndependentMax default heavy is **2:1**, not `k/7`. `--braid-chance 2..6`
  still uses `k/7`; collectors with `braid_numer=7` remain always-heavy.
- `--zag-chance` / `--bead-chance` IndependentMax stuffing is unchanged
  (default 1 → weights 3 and 1 in 5:3:1).
- Independent channels was already the default on `main`; this PR adds
  `--independent-channels on|off` and A/B benches rather than flipping
  the live default.
- 9.69% braid rings is inside 8–12%. No further heavy-coin tweak.
- Light H2H still uses `--zag-mismatch sum` (IndependentMax takes the
  max of that component).
- Additive 7:5:4 made control-5 *easier* for shallow bots, not harder.
  If the design goal is more counterplay around an early center holder,
  channel mode (IndependentMax) moves the needle more than the
  `counterplay` personality did at this budget.
- CLAIM-on-center is still dead. Local-claim dest-neighbor pops cover
  the adjacent case; leftover overwhelmed center is the remaining hole
  and search still does not take that extra ply.

---

# Round 2 — same-deck 27, beads-in-normal, braid ~10% of tiles, ColorPass.drift

Liam follow-up on PR #79. Same harness as round 1 except: `--same-deck on`
(both players begin with the **identical 27-tile collection**), mixed:mixed
only, `--games 50` × 2 pairings = 100 games, counterplay search, `--seed 1`,
`--budget nodes:33000,iters:650`, `--jobs 4`, `--no-default-features`, zags
on, `--zag-mismatch sum`, control-cascade, local-claim. Reproduce with
[`run_h2h_same_deck.sh`](run_h2h_same_deck.sh). ColorPass.drift diagonal
benches: [`run_h2h_color_drift.sh`](run_h2h_color_drift.sh). Survey:
[`survey_center.py`](survey_center.py) (prints `same_deck_27 identical=…`).

Do not overwrite round-1 `h2h_a_*` / `h2h_b_*` logs.

## Same-deck 27

`--same-deck on` is mixed:mixed + `identical_bags` + `deck_size=27` +
`deck_noise=0`. The 7+4 pick can still differ; the 27-tile **collection**
does not. Every round-2 JSONL is `same_deck_27 identical=100/100` (754
baseline and all 4:3:2 / 4:3:1 / drift logs). Opening benches match
(`opening_strength_equal=100/100` on 4:3:2 IndependentMax).

## Beads in the normal channel

Beads are not an extra scoring/render component. They fold into the
normal channel:

- IndependentMax: numeric max is associative, so scores are unchanged
  when bead is folded into normal before the max.
- Additive: `SharedChannel` = `max(normal, bead) + zag + braid`.
- Paint/HUD: bead discs stay on the **normal lane** when a Normal match
  is kept (`ArcStyle::lane_offsets` copies the normal offset onto the
  bead slot). The screenshot bug was zag winning a mismatch tie and
  dropping the bead lane.

## ChannelMode is scoring-only

Generation always IndependentMax clustering (per-slot 5:3 heavy coin +
5:3:1 light). `--channel-mode additive` / `--independent-channels off`
only changes how match/mismatch channels combine (sum vs max). No
Additive braid/bead 2–3 stuffing. Round-1 config A’s extra doubles/triples
were that stuffing; round-2 Additive uses the same tile mix as IndependentMax.

## Braid ~10% of tiles (not rings)

Default heavy coin is **5:3** light:braid (`p = 3/8 = 0.375`), not the
round-1 2:1. Target is ~10% of **tiles** with a consecutive-heavy braid
group, via the existing heavy-selection (not a separate 10% roll).

| Mix | n1 / n2 / n3 | braid tile / ring | decks with a braid |
|---|---|---|---|
| 4:3:2 | 44.4 / 32.8 / 22.7% | **9.59%** / 11.5% | 94.0% |
| 4:3:1 | 49.6 / 37.7 / 12.7% | 7.96% / 10.1% | 90.0% |
| 7:5:4 | 44.0 / 30.6 / 25.4% | **10.04%** / 11.8% | 94.6% |

4:3:2 hits the tile-rate band; 4:3:1 undershoots because there are fewer
2–3-ring tiles to braid. 4:3:1 is the triple cut Liam asked for (12.7%
vs 22.7% / 25.4%).

## ColorPass.drift (existing, currently zeroed)

No new generation mechanism. RFC 000 `ColorPass.drift` is a `[[i32;6];6]`
hysteresis matrix: after drawing color *i*, add `drift[i][j]` to weight
*j*. Live default is all zeros. `--color-drift N` sets the **diagonal**
to *N* (the RFC keep-last example is `+40`).

Default 0 keeps independent `sample_color` draws and rejects duplicate
colors on a tile, so deals match the pre-drift generator. Nonzero allows
repeats and applies the matrix. Collection generation keeps the same
running weights across the 27 tiles (same matrix, not a second setting).

**Parameter:** RFC `ColorPass.drift` diagonal via `--color-drift`.

**Values tested:** `0` (current / default), `40` (RFC moderate), `160` (strong).

Multicolor on mixed 27-tile collections, 4:3:2 (400 decks in-unit, plus
`# ring_process_deck_colors` on the H2H logs for 40 / 160):

| Diagonal | 2+ color tile share | mean distinct colors / tile | mean distinct colors / 27-tile deck |
|---|---|---|---|
| 0 | 0.557 | 1.79 | 5.03 |
| 40 | 0.390 | 1.44 | 3.85 |
| 160 | 0.284 | 1.31 | 3.00 |

At 0 every 2–3-ring tile is multicolor, so the share equals P(n≥2). 7:5:4
at 0 is 0.560 tiles / 5.00 colors per deck; 4:3:1 at 0 is 0.499 / 4.88.

## Same-deck H2H (quoted)

| Log | Mix | Channels | P1 W-D-L | Mean turns | lock / ctrl / LV / TL | opp/self cap | center opp cap | claims |
|---|---|---|---|---|---|---|---|---|
| [`h2h_same_432_imax`](h2h_same_432_imax.txt) | 4:3:2 | imax | **63-0-37** | 35.76 | 0 / 100 / 0 / 0 | 1.89 / 1.40 | 0.18 | 0.01 |
| [`h2h_same_432_add`](h2h_same_432_add.txt) | 4:3:2 | additive | 50-3-47 | **59.33** | 2 / 94 / 1 / 3 | 4.03 / 4.03 | 0.48 | 0.02 |
| [`h2h_same_431_imax`](h2h_same_431_imax.txt) | 4:3:1 | imax | 53-0-47 | 38.52 | 1 / 99 / 0 / 0 | 2.39 / 2.17 | 0.28 | 0.01 |
| [`h2h_same_431_add`](h2h_same_431_add.txt) | 4:3:1 | additive | 59-0-41 | 42.25 | **9** / 91 / 0 / 0 | 4.08 / 4.03 | **0.68** | 0.01 |
| [`h2h_same_754_imax`](h2h_same_754_imax.txt) | 7:5:4 | imax | 52-1-47 | 34.82 | 0 / 99 / 0 / 1 | 1.69 / 1.30 | 0.21 | 0.00 |

Control-cascade target5 / 4 / 3 / 2 (immediate vs hold):

| Log | t5 | t4 | t3 | t2 | immediate | hold |
|---|---|---|---|---|---|---|
| 432 imax | 99 | 1 | 0 | 0 | 1 | 99 |
| 432 add | 70 | 16 | 6 | 2 | 14 | 80 |
| 431 imax | 88 | 4 | 7 | 0 | 2 | 97 |
| 431 add | 64 | 13 | 8 | 6 | 18 | 73 |
| 754 imax | 95 | 1 | 2 | 1 | 1 | 98 |

On same-deck mixed:mixed, **Additive is the contested mode** (the opposite
of round 1’s braid:bead Additive sit-and-win). IndependentMax 4:3:2 and
7:5:4 are ~20-turn control-5 holds. Cutting triples (7:5:4 → 4:3:2 →
4:3:1) trims IndependentMax never-left 66% → 58% → 52% and raises center
opp caps 0.21 → 0.18 → 0.28. 4:3:1 IndependentMax is the most even imax;
4:3:2 Additive is the most even overall W-D-L; 4:3:1 Additive has the
most recapture and the only center CLAIM, but 9 lockouts and a 59% P1.

Drift benches used **4:3:2** (braid-tile target + the higher-multicolor
mix). IndependentMax and Additive both run at diagonal 0 / 40 / 160.

## Center-holder survey (same-deck)

| Log | First take | Still / never left | First-taker side wins | Still ⇒ that side wins | Vacates | Center opp caps |
|---|---|---|---|---|---|---|
| 432 imax | 100/100 @ t5.4 | 59% / **58%** | 61% | 47/59 = 80% | 43 | 18 |
| 432 add | 100 @ t6.5 | 49% / **45%** | 54% | 35/49 = 71% | 72 | 48 |
| 431 imax | 100 @ t5.4 | 54% / **52%** | 56% | 44/54 = 82% | 47 | 28 |
| 431 add | 99 @ t6.8 | 46% / **45%** | 52% | 32/45 = 71% | 51 | **68** |
| 754 imax | 100 @ t4.8 | 68% / **66%** | 55% | 45/68 = 66% | 36 | 21 |

### ColorPass.drift on 4:3:2 IndependentMax

| Diagonal | multi tiles | P1 W-D-L | Mean turns | Still / never left | First-taker wins | Still ⇒ win | Center opp cap |
|---|---|---|---|---|---|---|---|
| 0 | 55.6% | 63-0-37 | 35.76 | 59% / 58% | 61% | 80% | 0.18 |
| 40 | 39.0% | 56-0-44 | 37.64 | 61% / 60% | 63% | **88%** | 0.23 |
| 160 | 28.4% | 60-0-40 | 33.77 | 64% / **63%** | 54% | 66% | 0.24 |

Lowering multicolor via the existing drift diagonal does **not** buy
IndependentMax counterplay. Never-left rises 58% → 60% → 63%. Center
recapture barely moves (0.18 → 0.23 → 0.24). At 160 the first-taker’s
side wins less often (54%) because a sitting piece is more often outscored
on the 9-node control set (still⇒win 66%), not because the occupant is
popped.

### ColorPass.drift on 4:3:2 Additive

| Diagonal | multi tiles | P1 W-D-L | Mean turns | Still / never left | First-taker wins | Still ⇒ win | Center opp cap |
|---|---|---|---|---|---|---|---|
| 0 | 55.6% | 50-3-47 | **59.33** | 49% / **45%** | 54% | 71% | **0.48** |
| 40 | 39.0% | 60-0-40 | 42.86 | 61% / **59%** | 60% | 72% | 0.43 |
| 160 | 28.4% | 53-1-46 | 46.88 | 57% / **55%** | 59% | 81% | 0.42 |

Logs: [`h2h_same_432_add_drift40`](h2h_same_432_add_drift40.txt),
[`h2h_same_432_add_drift160`](h2h_same_432_add_drift160.txt)
(IndependentMax: [`drift40`](h2h_same_432_imax_drift40.txt),
[`drift160`](h2h_same_432_imax_drift160.txt)).

High-multicolor Additive (diagonal 0) is the **most** contested of the
six 4:3:2 drift cells: longest games, lowest never-left, most recapture,
even W-D-L. Turning drift up makes Additive look more like IndependentMax
sit-and-hold. Low-multicolor decks did **not** produce more counterplay
than high-multicolor ones at this budget.

## Qualitative

**Same 27, still sit-and-hold (4:3:2 IndependentMax).**
`h2h_same_432_imax` mcts vs minimax game 7: P1 MOVE onto center at t3,
zero vacates/captures/claims, control at t29. 58 of 100 games never
leave. Opening benches are identical; the 7+4 pick still gives P1 the
first MOVE.

**Additive same-deck actually fights (4:3:2).**
`h2h_same_432_add` mcts vs minimax game 40: 15 center occupants, 12
vacates, 3 recaptures, 200-turn draw. Mean turns 59, never-left 45%,
center opp caps 0.48. Three turn-limit games. This is the reverse of
round-1 Additive braid:bead.

**4:3:1 Additive recapture, including a center CLAIM.**
`h2h_same_431_add` mcts vs minimax game 24: P2 takes center t6, six
occupants, four vacates, one recapture, **CLAIM on the center**, P2
wins control at t148. Only center CLAIM in the round-2 matrix. Nine
lockouts in this config (the 4:3:1 Additive tax).

**7:5:4 same-deck IndependentMax is the stickiest.**
`h2h_same_754_imax` mcts vs minimax game 1: P2 MOVE t4, still there at
t20 control. 66% never-left, first take at t4.8.

**ColorPass.drift 40, IndependentMax: P1 less dominant, occupant still sits.**
`h2h_same_432_imax_drift40` P1 56-0-44 (was 63-0-37) but never-left 60%.
minimax vs mcts game 35: eight occupants, a center CLAIM, 111 turns —
the rare contest. Typical game is still a t4–t6 MOVE that stays.

**ColorPass.drift 160 IndependentMax: fewer colors, more sit, occasional long shuffle.**
`h2h_same_432_imax_drift160` mcts vs minimax game 9: 10 occupants, 8
vacates, 137 turns, P1 wins after P2’s first take is displaced. Mean
never-left is still 63%. Deck mean distinct colors is 3.00.

**ColorPass.drift 40 Additive: the even Additive game gets P1-heavier and stickier.**
`h2h_same_432_add_drift40` P1 60-0-40 (was 50-3-47), never-left 59% (was
45%), mean turns 43 (was 59). mcts vs minimax game 5: P2 MOVE t4, still
there at t20.

**ColorPass.drift 160 Additive: still stickier than diagonal 0, with rare melees.**
`h2h_same_432_add_drift160` never-left 55%, center opp cap 0.42. minimax
vs mcts game 2: 19 occupants, 11 vacates, 5 recaptures, P2 wins control
at t143 — the exception, not the mean.

## Open design questions (round 2)

- Heavy coin is **5:3**, not 2:1 and not `k/7`. `--braid-chance 2..6`
  still uses `k/7`.
- 4:3:2 hits ~10% braid **tiles**; 4:3:1 does not (8.0%). Triples at
  4:3:1 are 12.7% vs 22.7% at 4:3:2. If both targets matter, the mix
  is still open.
- Same-deck mixed:mixed **flips** the round-1 Additive vs IndependentMax
  story. Additive is the counterplay mode here; IndependentMax sits.
- RFC ColorPass.drift diagonal 40 / 160 cuts 2+ color tiles 56% → 39%
  → 28% and colors/deck 5.0 → 3.8 → 3.0. It does **not** cut never-left
  on either channel mode; Additive counterplay is **highest** at
  diagonal 0. Multicolor is not the sit-and-win lever at this budget —
  if anything, fewer colors made Additive stickier.
- CLAIM-on-center appeared once (4:3:1 Additive game 24, and once at
  drift 40 IndependentMax). Still not a plan the bots pursue.
- Default drift stays 0 so deals are unchanged. Nonzero is a bench
  knob (`--color-drift`), not a live default flip.
