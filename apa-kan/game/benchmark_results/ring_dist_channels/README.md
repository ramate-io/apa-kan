# Ring-count 7:5:4, ~10% braid rings, independent channels, counterplay search

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
