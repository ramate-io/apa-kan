# Center link-once, Additive beads, braid-only-match

Stacked on PR #79 (same-deck 27, ColorPass.drift). Playtesting and those
logs showed too little counterplay: the first player onto the center tends
to sit and win. Liam’s diagnosis is that it is too hard to overwhelm,
especially around the center; a tweak that makes overwhelming a trade is
still better than stasis because it forces a reaction.

This package measures three rule levers, all saveable / CLI-flagged:

1. **Additive beads are a separate channel.** Undo PR #79’s Additive
   `max(normal, bead)` fold. Each firing channel (normal, bead, zag, braid)
   contributes its weight. IndependentMax is unchanged (max of the four
   components). `--bead-scoring shared` restores the fold.
2. **Center↔inner counted once.** `--link-once on` /
   `--score-unique-neighbors on`. An edge between two unique nodes
   contributes once to match/mismatch. Default remains per-arc (two petals
   between center and each inner node).
3. **`--braid-scoring braid-only-match`.** Only a fulfilled braid adds to
   the edge **match** total. Other matching channels still occupy (they are
   not hitches) and are still **painted** as matches in the HUD; they score
   0. Mismatches are unchanged. Default `--braid-scoring current` is live
   scoring.

Rarer multicolor uses the existing `--color-drift 160` diagonal (no new
drift mechanism). PR #79 measured ~28% 2+ color tiles at 160 vs ~56% at 0;
this package’s `# ring_process_multicolor` is **24.4%** (mean 1.25 distinct
colors / tile, 2.97 / 27-tile deck).

Reproduce with [`run_h2h.sh`](run_h2h.sh). Survey:
[`survey.py`](survey.py) (first center *take* counts only if the landing
piece is still there after that action’s captures resolve).

Do not overwrite these `h2h_*` logs.

## Package

Release `apa-kan-bench`, `--no-default-features`, `--budget nodes:33000,iters:650`,
`--games 50`, `--jobs 4`, `--no-depth-probe`, `--zags on`, `--zag-mismatch sum`,
`--endgame-rule control-cascade`, `--cascade-turn-gain on`,
`--capture-mode local-claim`, `--no-same-piece-twice off`, `--pick search`,
`--pairings minimax:mcts,mcts:minimax`, `--same-deck on`,
`--search-profile counterplay`, `--seed 1`, `--color-drift 160`,
`--link-once on`. 50 games × 2 pairings = **100 games** per config.
Singles swap is off (not passed; default off).

Two matrices, each `(4:3:1, 7:5:4) × {independent-max, additive}`:

| Log | Mix | Channels | Braid scoring |
|---|---|---|---|
| [`h2h_431_imax`](h2h_431_imax.txt) | 4:3:1 | independent-max | current |
| [`h2h_431_add`](h2h_431_add.txt) | 4:3:1 | additive | current |
| [`h2h_754_imax`](h2h_754_imax.txt) | 7:5:4 | independent-max | current |
| [`h2h_754_add`](h2h_754_add.txt) | 7:5:4 | additive | current |
| [`h2h_431_imax_bom`](h2h_431_imax_bom.txt) | 4:3:1 | independent-max | braid-only-match |
| [`h2h_431_add_bom`](h2h_431_add_bom.txt) | 4:3:1 | additive | braid-only-match |
| [`h2h_754_imax_bom`](h2h_754_imax_bom.txt) | 7:5:4 | independent-max | braid-only-match |
| [`h2h_754_add_bom`](h2h_754_add_bom.txt) | 7:5:4 | additive | braid-only-match |

`run_h2h.sh matrix` is (a); `run_h2h.sh braid` is (b); `run_h2h.sh all` is both.

## How braid-only-match is interpreted

Channel *components* are unchanged: a normal match is still
`normal_match = 1` (not a hitch); a matched bead is still `bead_match = 1`;
a fulfilled braid is still `braid_match =` its slot count; zag / failed-braid
/ unmatched-bead mismatches are the live amounts.

Aggregation of the **match total**:

- `current` + IndependentMax: `max(normal_match, bead_match, braid_match)`
- `current` + Additive Independent: `normal_match + bead_match + braid_match`
  (beads-plus-normals on the same edge is a 2-match event again)
- `braid-only-match` (either channel mode): match total = `braid_match` only

A 1-ring tile cannot form a 2-slot braid, so under braid-only-match its
matching edges occupy and display a match but contribute 0. Occupancy still
blocks a hitch that a mismatching placement would have scored.

HUD: Additive paints every firing channel, including non-braid matches.
IndependentMax HUD still keeps only the two winning component lanes; a
normal match that scores 0 is still shown when it is the max-match
component (no braid on the edge).

`exact` and `double-edged` were dropped; they are not in this PR.

## Unique-neighbor scoring (link-once)

`Board::incident_others` walks generating-circle arcs, so the eight
center↔inner pairs are each two petals. `Board::neighbors` is the unique
node pair. `--link-once on` scores `neighbors`.

Verification (`only_center_inner_pairs_have_two_arcs`): every node pair
has ≤ 2 connecting arcs. **Exactly the eight center↔inner pairs have 2.**
No other pair is double-counted. Inner-ring nodes still have 4 incident
arcs and 3 unique neighbors (the extra arc is the second petal to the
origin). Spawn–inner, inner–inner, and outer pairs are already unique.

JSONL: `score_unique_neighbors: true` and `score_incident_arcs: false`.
Older logs omit the field and replay as per-arc.

## Additive beads (Independent)

Default `--bead-scoring independent`. Additive match =
`normal + bead + braid`; mismatch = `normal + bead + zag + braid`.
IndependentMax maxes the four components either way. HUD: Additive
Independent paints beads on their own lane; IndependentMax still folds
bead discs onto the normal lane.

## Baselines (PR #79 same-deck, link double-count, drift 0)

Re-surveyed with this package’s take definition (survive the landing
action’s captures). Additive in PR #79 used the **folded**
`max(normal, bead)` channel, so `h2h_same_431_add` is not a matching
Additive cell — only the mix / channel-mode name. There is no same-deck
7:5:4 Additive log on PR #79.

| Log | Mix | Ch | P1 W-D-L | Turns | Ends (lock/ctrl/LV/TL) | Never left | First-taker wins | Opp cap | Mutual | Holder→inner / reverse | ZB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| [`../ring_dist_channels/h2h_same_431_imax`](../ring_dist_channels/h2h_same_431_imax.txt) | 4:3:1 | imax | 53-0-47 | 38.52 | 1 / 99 / 0 / 0 | 54.0% | 56.0% | 0.28 | 85.7% | 0.77 / 0.23 | 27.6% |
| [`../ring_dist_channels/h2h_same_431_add`](../ring_dist_channels/h2h_same_431_add.txt) | 4:3:1 | add† | 59-0-41 | 42.25 | 9 / 91 / 0 / 0 | 45.5% | 51.5% | 0.68 | 86.8% | 1.04 / 0.37 | 24.9% |
| [`../ring_dist_channels/h2h_same_754_imax`](../ring_dist_channels/h2h_same_754_imax.txt) | 7:5:4 | imax | 52-1-47 | 34.82 | 0 / 99 / 0 / 1 | 67.0% | 54.0% | 0.21 | 85.7% | 0.68 / 0.17 | 23.4% |
| [`../ring_dist_channels/h2h_same_432_imax_drift160`](../ring_dist_channels/h2h_same_432_imax_drift160.txt) | 4:3:2 | imax | 60-0-40 | 33.77 | 0 / 100 / 0 / 0 | 63.0% | 54.0% | 0.24 | 70.8% | 0.66 / 0.20 | 25.7% |
| [`../ring_dist_channels/h2h_same_432_add_drift160`](../ring_dist_channels/h2h_same_432_add_drift160.txt) | 4:3:2 | add† | 53-1-46 | 46.88 | 4 / 95 / 0 / 1 | 56.0% | 58.0% | 0.42 | 88.1% | 1.01 / 0.46 | 33.5% |

† PR #79 Additive = SharedChannel fold. First take is always MOVE, mean turn
4.8–6.9. CLAIM-on-center is still ~0.

## Head-to-head (this package)

Same-deck 27 identical=100/100 on every log. First take is MOVE in every
recorded take (two Additive cells missed a take in 2/100 games).

### (a) Current braid scoring, link-once, drift 160

| Log | P1 W-D-L | Turns | Ends (lock/ctrl/other) | First take | Never left | First-taker wins | Opp cap | Mutual | Holder→inner / reverse | ZB |
|---|---|---|---|---|---|---|---|---|---|---|
| 431 imax | 59-0-41 | 30.14 | 2 / 98 / 0 | 100 @ t4.14 | **75.0%** | 65.0% | 0.19 | 47.4% | 0.69 / 0.21 | 14.9% |
| 431 add | 63-0-37 | 38.99 | 7 / 93 / 0 | 98 @ t6.03 | 62.2% | 60.2% | 0.32 | 84.4% | 1.06 / 0.47 | 26.6% |
| 754 imax | 62-0-38 | 28.30 | 0 / 99 / 1 LV | 100 @ t3.59 | **83.0%** | 69.0% | 0.12 | 41.7% | 0.42 / 0.22 | 13.9% |
| 754 add | 59-0-41 | 30.51 | 0 / 99 / 1 elim | 100 @ t4.62 | 71.0% | 65.0% | 0.30 | 76.7% | 0.70 / 0.32 | 18.2% |

Control-cascade target5 / 4 / 3 / 2 (immediate vs hold):

| Log | t5 | t4 | t3 | t2 | immediate | hold |
|---|---|---|---|---|---|---|
| 431 imax | 92 | 3 | 2 | 1 | 5 | 93 |
| 431 add | 70 | 10 | 10 | 3 | 11 | 82 |
| 754 imax | 97 | 2 | 0 | 0 | 2 | 97 |
| 754 add | 90 | 3 | 5 | 1 | 6 | 93 |

Link-once + drift 160 **increased** sit-and-win versus the PR #79
double-count / drift-0 same mix. 4:3:1 IndependentMax never-left 54% → 75%;
7:5:4 IndependentMax 67% → 83%. Center opp caps fell. Mutual-trade share
on IndependentMax dropped (~86% → ~42%) because recaptures themselves
became rare. Counting the center petal once makes the holder harder to
overwhelm; rarer multicolor removes hitch diversity. Together they make
the “sit” worse, not better.

### (b) Braid-only-match, same link-once + drift 160

| Log | P1 W-D-L | Turns | Ends (lock/ctrl/other) | First take | Never left | First-taker wins | Opp cap | Mutual | Holder→inner / reverse | ZB |
|---|---|---|---|---|---|---|---|---|---|---|
| 431 imax bom | 64-0-36 | 43.80 | **22** / 78 / 0 | 99 @ t4.33 | 61.6% | **73.7%** | 0.26 | 42.3% | 1.61 / 0.66 | 20.8% |
| 431 add bom | **54-0-46** | 48.74 | **28** / 68 / 3 elim, 1 LV | 98 @ t7.35 | **38.8%** | 58.2% | **0.62** | 69.4% | 1.69 / 0.88 | 24.7% |
| 754 imax bom | 58-0-42 | 46.93 | 9 / 91 / 0 | 100 @ t4.04 | 70.0% | 70.0% | 0.19 | 47.4% | 1.52 / 0.71 | 22.4% |
| 754 add bom | 62-1-37 | 48.77 | 19 / 77 / 2 elim, 1 LV, 1 TL | 99 @ t6.04 | 54.5% | 73.7% | 0.44 | 75.0% | 2.01 / 0.82 | 23.3% |

| Log | t5 | t4 | t3 | t2 | immediate | hold |
|---|---|---|---|---|---|---|
| 431 imax bom | 56 | 9 | 10 | 3 | 9 | 69 |
| 431 add bom | **24** | 11 | 18 | 15 | 15 | 53 |
| 754 imax bom | 69 | 12 | 8 | 2 | 15 | 76 |
| 754 add bom | 43 | 12 | 13 | 9 | 15 | 62 |

**4:3:1 Additive braid-only-match is the only cell that clearly forces
reactions:** never-left 38.8% (vs 62.2% current / 45.5% PR #79 folded
Additive), contested-center 60%, even 54-46, center opp caps 0.62, ratchet
often drops below 5. Cost: 28 lockouts, 49-turn games, and the first-taker
side still wins 58%.

IndependentMax + braid-only-match lengthens games and raises inner-ring
captures (~2.5×) but the first-taker win rate *rises* (65% → 74% at 4:3:1;
69% → 70% at 7:5:4). More fighting around the ring does not flip the
center.

## By pairing / seat

Minimax-as-P1 is the seat that sits. MCTS-as-P1 is closer to even and
sometimes P2-favored.

| Log | mcts vs minimax P1 | never / first_wins / opp | minimax vs mcts P1 | never / first_wins / opp |
|---|---|---|---|---|
| 431 imax | 24-0-26 | 80% / 56% / 0.14 | 35-0-15 | 70% / 74% / 0.24 |
| 431 add | 24-0-26 | 59% / 47% / 0.36 | 39-0-11 | 65% / 74% / 0.28 |
| 754 imax | 18-0-32 | 78% / 50% / 0.18 | **44-0-6** | 88% / 88% / 0.06 |
| 754 add | 23-0-27 | 72% / 62% / 0.34 | 36-0-14 | 70% / 68% / 0.26 |
| 431 imax bom | 22-0-28 | 49% / 63% / 0.26 | **42-0-8** | 74% / 84% / 0.26 |
| 431 add bom | 19-0-31 | **35%** / 55% / 0.64 | 35-0-15 | 43% / 61% / 0.60 |
| 754 imax bom | 13-0-37 | 66% / 54% / 0.24 | **45-0-5** | 74% / 86% / 0.14 |
| 754 add bom | 17-1-32 | 43% / 59% / 0.48 | **45-0-5** | 66% / 88% / 0.40 |

## Singles under braid-only-match

Liam’s suspicion: a 1-ring tile becomes a spend-once piece (placed once,
then useless), because it cannot make a braid match.

Placed pieces with `n_rings == 1`, compared to the same mix’s current-scoring
cell. “Never moved” = no MOVE after the placing ADD. “Healed” = the
piece’s action recorded `healed > 0` (occupancy that saved an overwhelmed
neighbor). That count is ~0 everywhere, including current scoring.

| Log | Placed | Moved later | Never moved | Mean moves | Captured w/o move | Survived | Actor opp cap | Touched control |
|---|---|---|---|---|---|---|---|---|
| 431 imax current | 92 | 48.9% | 51.1% | 1.09 | 17.4% | 44.6% | 13.0% | 42.4% |
| 431 imax bom | 102 | **22.5%** | **77.5%** | **0.25** | 23.5% | **2.9%** | 22.5% | **8.8%** |
| 431 add current | 115 | 60.9% | 39.1% | 1.62 | 22.6% | 24.3% | 34.8% | 41.7% |
| 431 add bom | 120 | 56.7% | 43.3% | 1.16 | 20.0% | 9.2% | 38.3% | 35.8% |
| 754 imax current | 71 | 57.7% | 42.3% | 1.25 | 8.5% | 62.0% | 21.1% | 45.1% |
| 754 imax bom | 78 | 33.3% | **66.7%** | 0.92 | 17.9% | 7.7% | 17.9% | 14.1% |
| 754 add current | 84 | 70.2% | 29.8% | 1.07 | 8.3% | 57.1% | 20.2% | 54.8% |
| 754 add bom | 87 | 48.3% | 51.7% | 1.46 | 21.8% | 11.5% | 34.5% | 27.6% |

IndependentMax + braid-only-match matches the suspicion: singles almost
stop being repositioned, almost never sit on control, and almost never
survive. They still capture on the placing action (~18–23%) — typically a
spawn/outer pop, not a center fight. Never-moved singles that touched
control: **0** in every BOM log.

Additive + braid-only-match does **not** freeze singles the same way.
Move rate stays near the current-scoring Additive cell. Survival drops
(more captures overall) but they still get reused.

Composition of placed singles is mostly normal + bead; zags are 5–30.
Bead singles under braid-only-match score 0 even when the bead “matches,”
so IndependentMax has no reason to walk them onto a control node.

PR #79 current IndependentMax 4:3:1 (for scale): singles moved later 53.4%,
never moved 46.6%, mean moves 0.92, touched control 39.8% — closer to this
package’s current IndependentMax than to BOM.

## Qualitative log review

Citations are `log` / `pairing` / `game` / `turns`. Inner-ring node ids
are `{9,1,12,3,16,6,18,21}`; center is 0. Every first take in these 800
games is a MOVE (usually from an inner or near-inner node on turns 3–8).
CLAIM-on-center is still 0.

### Current scoring — sit is the main line

IndependentMax games are short control-5 holds. Example sit-and-win:
[`h2h_431_imax`](h2h_431_imax.jsonl.gz) / mcts vs minimax / game 1 / 26
turns. P2 MOVEs 3→0 at t6 with a 2-normal and never leaves; P1’s only
nearby event is P2 popping inner 3 at t16. Same pattern at 7:5:4:
[`h2h_754_imax`](h2h_754_imax.jsonl.gz) / mcts vs minimax / game 2 / 20
turns, P2 5→0 at t4, then one inner pop of P1 at t10, control-5.

When the first piece *does* leave, it is often a voluntary vacate, not a
pop. [`h2h_431_imax`](h2h_431_imax.jsonl.gz) / mcts vs minimax / game 2 /
25 turns: P2 takes 1→0 at t8, walks off 0→1 at t24, P1 wins the empty
center. The 147-turn exception
[`h2h_431_imax`](h2h_431_imax.jsonl.gz) / minimax vs mcts / game 44 is
hot-potato at ratchet 4: the same 2-normals walk on and off the origin
from t35 to t146, almost all at budget 0, one mutual recapture at t32–33.
That is stalling, not overwhelm.

The recapture that exists is the inner-adjacent mutual trade. Example:
[`h2h_431_imax`](h2h_431_imax.jsonl.gz) / mcts vs minimax / game 7 / 28
turns. P1 takes t5; P2 MOVEs onto inner 1 at t6 and pops center+self;
P2 retakes 20→0 at t8 and sits. Additive current scoring does this more
often (0.32 opp caps/game) but still 62% never-left. Example two-way inner
fight: [`h2h_431_add`](h2h_431_add.jsonl.gz) / mcts vs minimax / game 6 /
48 turns — P2 takes at t14, P1 pops the center from inner 9 at t33
(not mutual), P2 retakes and pops P1’s 9, ratchet falls to 3, P2 walks
off and back onto 0 at t45–47. Rare, and P1 still won.

7:5:4 Additive current is the same sit with slightly more recapture
(0.30). [`h2h_754_add`](h2h_754_add.jsonl.gz) / mcts vs minimax / game 1 /
40 turns is two inner-adjacent pops of the center (t13, t21) then an
immediate cascade win at target 3. The 12-holder
[`h2h_754_add`](h2h_754_add.jsonl.gz) / mcts vs minimax / game 38 / 145
turns is again zero-budget walking on/off 0, not a capture war.

Non-holder inner captures stay low (0.21–0.47/game). The holder pops
inner neighbors ~2–3× as often. Zero-budget share fell vs PR #79 on
IndependentMax (27.6% → 14.9% at 4:3:1) because the games ended before
the tank emptied.

### Braid-only-match — more ring fighting, lockouts, and one real contest cell

IndependentMax BOM still sits, just longer, and P1 lockouts appear.
[`h2h_431_imax_bom`](h2h_431_imax_bom.jsonl.gz) / mcts vs minimax / game 6 /
24 turns: P2 wipes two inners at t4, takes center at t8 with a 2-braid,
drops 1-bead singles on outer nodes at t12, pops another inner at t16,
P1 is locked out. The 1-bead never moves onto control.

A fight that actually uses the inner ring:
[`h2h_431_imax_bom`](h2h_431_imax_bom.jsonl.gz) / mcts vs minimax / game 16 /
113 turns. P2 takes t4; P1 pops that center from inner 6 at t19 (mutual
with self on 6), retakes t23, then farms P2’s inner nodes (12, 18, 16, 21)
while the origin changes hands at t79/t82/t87/t90 and P1 wins an immediate
drop at target 3. That is the counterplay Liam wants, and it is the
exception (never-left still 62%, first-taker wins 74%).

Singles in the sit-and-win
[`h2h_431_imax_bom`](h2h_431_imax_bom.jsonl.gz) / mcts vs minimax / game 1 /
59 turns: three 1-bead ADDs at t19/t22/t28 onto outer/spawn nodes; one of
them shuffles 11→14→19 and self-pops. None enter `{0}∪inner`.

**4:3:1 Additive BOM** is the cell where the non-holder regularly answers.
[`h2h_431_add_bom`](h2h_431_add_bom.jsonl.gz) / mcts vs minimax / game 7 /
31 turns: P1 takes t5; P2 pops it from inner 18 at t12 (mutual); P1
retakes t13; P2 pops from 12 at t16; P2 retakes t18; P1 pops from 12 at
t19. Three center trades in 14 turns, ratchet to 2, P1 holds. Same
script at 7:5:4 Additive BOM:
[`h2h_754_add_bom`](h2h_754_add_bom.jsonl.gz) / mcts vs minimax / game 21 /
29 turns, four inner-adjacent recaptures, ratchet 5→2.

The holder can still sit through a long game if the non-holder never finds
an adjacent hitch:
[`h2h_431_add_bom`](h2h_431_add_bom.jsonl.gz) / mcts vs minimax / game 8 /
157 turns, never-left, four holder→inner pops, control-5.

Lockouts (28 at 4:3:1 Additive BOM, 22 at IndependentMax BOM) often look
like the holder occupied enough of the 9-node set that the opponent ran
out of legal ADD/MOVE while the ratchet never dropped. That is a different
failure mode from “can’t overwhelm the center piece.”

Non-holder inner captures rise under BOM (0.66–0.88 vs 0.21–0.47) but the
holder’s inner pops rise too (1.52–2.01). The ring is hotter; the origin
is not equally available.

## Open design questions

- **Link-once vs overwhelm.** Counting the center petal once is the
  clean unique-pair rule, and no other pair was double-counted. It also
  *halves* hitch/match on the most important edges. Stacked with drift 160
  it made current-scoring sit-and-win worse. Should the live default stay
  per-arc, use link-once everywhere, or weight center↔inner by something
  other than 1 or 2?
- **Which lever hurt.** This package stacked link-once, drift 160, Additive
  unfold, and (in matrix b) braid-only-match. There is no cell that is
  link-once + drift 0, or drift 160 + double-count, at 4:3:1 / 7:5:4. The
  4:3:2 drift-160 PR #79 logs still sat (never-left 63% imax / 56% add).
- **Braid-only-match lockouts.** 9–28% of BOM games are `locked_out` vs
  0–7% current. Is that an acceptable “force a reaction” cost, or did
  occupancy-without-match starve legal moves?
- **Singles.** IndependentMax + braid-only-match makes 1-rings spend-once.
  Additive BOM does not, as much. Should a matching single still score +1,
  score a token occupancy bonus, or stay as hitch-blocker only?
- **First-taker win rate.** Lower never-left does not imply the non-holder
  wins. 4:3:1 Additive BOM never-left 39% but first-taker side still 58%.
  4:3:1 IndependentMax BOM never-left 62% and first-taker 74%. Is the
  goal “center changes hands” or “the second player can win”?
- **Minimax-as-P1.** Several BOM IndependentMax seats are 42–45 wins in 50
  for minimax P1. Shallow counterplay search may be over-valuing the sit.
- **CLAIM** is still dead (0 center claims in 800 games). Inner-adjacent
  MOVE is the only recapture grammar.
- **Hot-potato vs overwhelm.** Long contested games are mostly voluntary
  vacates at budget 0, not trades. Liam preferred trades to stasis; BOM
  Additive produces more trades *and* more walking.
- **Live default.** `--braid-scoring` defaults to `current`;
  `--link-once` defaults to off; `--bead-scoring` defaults to
  `independent` (Additive unfold). Should any of those flip for play?
- **`exact` / `double-edged`.** Dropped for now. Exact would reintroduce
  1-slot braids as +1 (a possible singles fix). Double-edged would make
  failed braids mismatch at full weight (more trades, more self-pops).
