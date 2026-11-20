# Single-braid exact match, normal-only swap, link-once vs double-count

Stacked on PR #82 (center link-once, Additive beads unfolded, braid-only-match)
after merging PR #80 (adjacent singles-swap). Liam wants to test the most
promising setup for restoring counterplay around the center: the first
center-taker still tends to sit and win. PR #82’s best cell was **4:3:1
Additive braid-only-match** (never-left 38.8%, 54-0-46, 28 lockouts,
first-taker wins 58.2%). This package layers single-braid exact matching
and normal-only swap on that recipe, and benches **both** center↔inner
link-once and the pre-#82 double-count.

All levers are saveable `RuleSet` / CLI flags. At the time of the cells
below, live defaults were still `--braid-scoring current`, `--single-braids
off`, `--singles-swap on` (from PR #80), `--link-once off`. Later in this
PR those live defaults flipped to the **full double-count** cell (4:3:1
Additive braid-only-exact, single braids on, normal-only swap, link-once
off, color-drift 160). Confirmation on a no-extra-rule-flag run:
[`h2h_defaults`](h2h_defaults.txt). Do not treat that as a rewrite of
the tables below.

Reproduce with [`run_h2h.sh`](run_h2h.sh). Survey: [`survey.py`](survey.py)
(first center *take* counts only if the landing piece is still there after
that action’s captures resolve).

Do not overwrite these `h2h_*` logs.

## What changed

1. **Ring-count weights 4:3:1** and **Additive** scoring (each firing
   channel contributes; beads are a separate Additive channel, as in #82).
2. **`--braid-scoring braid-only-exact`** (alias `--braid-exact on`).
   Like `braid-only-match`, only a fulfilled braid adds to the edge
   **match** total; other matching channels still occupy and paint, score 0;
   mismatches are unchanged. Fulfillment is **exact**: a braid matches only
   if the neighbor has exactly the braid’s count of each color present on
   the braid (`need[c] > 0 ⇒ have[c] == need[c]`). Extra neighbor colors
   the braid does not use are allowed. A 1-slot braid may match and scores
   **+1**. Coverage (`current` / `braid-only-match`) still requires ≥2
   slots and `have[c] ≥ need[c]`.
3. **`--single-braids on`.** A 1-ring lone heavy becomes a 1-slot braid
   using the existing 5:3 light:heavy coin (no extra roll). Lone heavies
   on 2–3 ring tiles and gapped doubles still fall through to light
   election. 2+ consecutive heavies are still 2–3 slot braids.
4. **`--singles-swap normal-only`.** Only a 1-ring **normal** may initiate
   SWAP (single braids / beads / zags cannot). Same adjacent-any, one-move
   no-echo, vacant center is not a partner (PR #80).
5. **Center↔inner link counting:** `--link-once on` (unique node pair) vs
   `--link-once off` (two petals / pre-#82 double-count).

Kept from the #82 recipe: `--same-deck on`, `--color-drift 160`, zags on,
doubles allowed, `--zag-mismatch sum`, counterplay search profile.

## Generation shares (5:3 heavy coin, no extra roll)

`# ring_process` sample=20000, 4:3:1, color-drift 160, single-braids **on**:

| | Tile braid | Single-braid | 2–3 slot braid | 1-ring tiles |
|---|---|---|---|---|
| Generated | **26.88%** | **18.66%** | ~8.2% | 49.5% |
| 27-tile decks | 27.01% | 18.81% | | almost every deck has a braid |

Without `--single-braids` (nobraid ablation) braid tiles return to **8.30%**
(the ~10% the 5:3 coin was tuned for) and `single_braid=0`.

Search pick (`--pick search`) then **dumps 1-rings into the 16-tile
reserves**. Opening 11-tile benches in the full-setup logs: **8.5%** 1-ring,
**0.5%** single-braid (10 single-braid tiles across 200 benches). The bag
is 75.8% 1-ring and 30.4% single-braid. Only **5/100** games per cell ever
place a single braid. The 5:3 coin with singles on overshoots the ~10%
braid target; pick then hides almost all of that overshoot from play.

## Package

Release `apa-kan-bench`, `--no-default-features`, `--budget nodes:33000,iters:650`,
`--games 50`, `--jobs 4`, `--no-depth-probe`, `--zags on`, `--zag-mismatch sum`,
`--endgame-rule control-cascade`, `--cascade-turn-gain on`,
`--capture-mode local-claim`, `--no-same-piece-twice off`, `--pick search`,
`--pairings minimax:mcts,mcts:minimax`, `--same-deck on`,
`--search-profile counterplay`, `--seed 1`, `--color-drift 160`,
`--independent-channels off`, `--ring-count-weights 4,3,1`,
`--braid-scoring braid-only-exact`. 50 games × 2 pairings = **100 games**
per config.

| Log | Link | Single braids | Swap | Notes |
|---|---|---|---|---|
| [`h2h_full_link_once`](h2h_full_link_once.txt) | once | on | normal-only | full setup |
| [`h2h_full_double`](h2h_full_double.txt) | double | on | normal-only | full setup |
| [`h2h_full_double_noswap`](h2h_full_double_noswap.txt) | double | on | off | ablation |
| [`h2h_full_double_nobraid`](h2h_full_double_nobraid.txt) | double | off | normal-only | ablation; 8.3% braid tiles |
| [`h2h_defaults`](h2h_defaults.txt) | double | on | normal-only | live RuleSet defaults; no extra rule flags |

`run_h2h.sh all` is the two full cells; `LINK=off run_h2h.sh noswap|nobraid`
are the ablations at double-count (the better-looking link setting).
`run_h2h.sh defaults` is the same harness on live `RuleSet::default()` with
no extra `--braid-scoring` / `--single-braids` / `--singles-swap` /
`--ring-count-weights` / `--color-drift` / `--independent-channels` /
`--link-once` flags.

JSONL.gz logs reconstruct each game (rules include `braid_scoring`,
`allow_single_braids`, `swap_mode`, `score_unique_neighbors`).

## Head-to-head vs PR #82 4:3:1 Additive BOM

Same-deck 27 identical=100/100 on every log. First take is MOVE in every
recorded take (none by ADD or SWAP). CLAIM-on-center is still ~0.

| Log | P1 W-D-L | Turns | Ends (lock/ctrl/other) | First take | Never left | First-taker wins | Opp cap | Mutual | Holder→inner / reverse | Swaps | ZB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| [#82 431 add bom](../center_link_once/h2h_431_add_bom.txt) | 54-0-46 | 48.74 | **28** / 68 / 4 | 98 @ t7.35 | 38.8% | 58.2% | 0.62 | 69.4% | 1.69 / 0.88 | 0 | 24.7% |
| full **link-once** | 53-0-47 | 44.94 | 32 / 63 / 5 | 98 @ t6.82 | 43.9% | 59.2% | 0.63 | 79.4% | 2.00 / 0.96 | 0.08 | 24.3% |
| full **double-count** | 54-0-46 | 52.71 | 35 / 61 / 4 | 95 @ t10.56 | **23.2%** | **55.8%** | **0.81** | 88.9% | 1.45 / 0.77 | 0.18 | 30.2% |
| double **no-swap** | 60-0-40 | 51.57 | 29 / 68 / 3 | 95 @ t10.29 | 26.3% | **50.5%** | 0.73 | 86.3% | 1.27 / 0.76 | 0 | 29.1% |
| double **no-single-braid** | **47-0-53** | 57.90 | **26** / 67 / 7 | 92 @ t9.30 | **22.8%** | **51.1%** | **0.96** | 84.4% | 1.62 / 0.82 | 0.18 | 29.9% |
| live **defaults** | 59-0-41 | 53.93 | 28 / 66 / 6 | 96 @ t10.16 | 26.0% | 51.0% | 0.82 | 90.2% | 1.35 / 0.84 | 0.16 | 27.1% |

Contested-center / sit-and-win (among games with a surviving take):

| Log | Contested | Sit-and-win | Dislodging swaps/game |
|---|---|---|---|
| full link-once | 56.1% | 39.8% | 0.02 |
| full double | **75.8%** | **20.0%** | 0.05 |
| double no-swap | 71.6% | 21.1% | 0 |
| double no-single-braid | **77.2%** | **18.5%** | 0.02 |
| live defaults | 74.0% | 22.9% | 0.07 |

Live `RuleSet::default()` confirmation (`h2h_defaults`) is in line with the
full double-count cell (never-left 26% vs 23%, sit-and-win 23% vs 20%,
swaps 0.16 vs 0.18). P1 is a bit stronger (59-0-41 vs 54-0-46) and more
1-rings reach play (opening 9.6% vs 8.5%; 74 single-braids placed vs a
handful in the original cell) because search pick rollouts now see exact
1-slot +1. Do not overwrite the original `h2h_full_*` logs.

Control-cascade target5 / 4 / 3 / 2 (immediate vs hold):

| Log | t5 | t4 | t3 | t2 | immediate | hold |
|---|---|---|---|---|---|---|
| #82 431 add bom | 24 | 11 | 18 | 15 | 15 | 53 |
| full link-once | 27 | 15 | 11 | 10 | 11 | 52 |
| full double | 26 | 6 | 13 | 16 | 18 | 43 |
| double no-swap | 28 | 7 | 15 | 18 | 19 | 49 |
| double no-single-braid | **20** | 13 | 17 | 17 | 22 | 45 |

**Double-count, not link-once, is the counterplay cell.** Full setup +
link-once is a wash vs #82 (never-left 44% vs 39%, same 53–54 P1, slightly
more lockouts). Full setup + **double-count** drops never-left to 23%,
sit-and-win to 20%, contested to 76%, first take later (t10.6, and 5 games
never land a surviving take). Cost: 35 lockouts and 30% zero-budget
actions.

The ablations at double-count say the new toys are not doing that work:

- **No-swap** is almost the same contestation (never-left 26%, sit-and-win
  21%) with fewer lockouts (29) and a more even first-taker win rate
  (50.5%), but P1 seat jumps to 60-0-40. SWAP fires 0.18/game when allowed.
- **No-single-braid** is as good or better: never-left 23%, first-taker
  51%, opp caps 0.96, lockouts 26, P1 47-0-53. Exact matching of *2-slot*
  braids plus restoring the two-arc center petal is the lever. Single
  braids barely enter play (see below).

## By pairing / seat

Minimax-as-P1 still sits more. MCTS-as-P1 is even or P2-favored.

| Log | mcts vs minimax P1 | never / first_wins / opp / swaps | minimax vs mcts P1 | never / first_wins / opp / swaps |
|---|---|---|---|---|
| full link-once | 17-0-33 | 41% / 47% / 0.54 / 0.10 | 36-0-14 | 47% / 71% / 0.72 / 0.06 |
| full double | 21-0-29 | 22% / 48% / 0.72 / 0.10 | 33-0-17 | 25% / 63% / 0.90 / 0.26 |
| double no-swap | **25-0-25** | 21% / 38% / 0.68 / 0 | 35-0-15 | 32% / 64% / 0.78 / 0 |
| double no-single-braid | 18-0-32 | 28% / 53% / 0.86 / 0.04 | 29-0-21 | **18%** / 49% / 1.06 / 0.32 |

## Single-normal and single-braid lives

Placed pieces with `n_rings == 1`. “Moved later” = a MOVE or SWAP after the
placing ADD. “Swapped” = that piece initiated at least one SWAP.

| Log | Kind | Placed | Moved later | Survived | Touched control | Swapped |
|---|---|---|---|---|---|---|
| full link-once | normal | 47 | 51.1% | 8.5% | 40.4% | 12.8% |
| full link-once | braid | 9 | 88.9% | 0% | 44.4% | 0% |
| full double | normal | 45 | 53.3% | 15.6% | 40.0% | 20.0% |
| full double | braid | 8 | 75.0% | 0% | 62.5% | 0% |
| double no-swap | normal | 43 | 53.5% | 14.0% | 44.2% | 0% |
| double no-swap | braid | 7 | 57.1% | 14.3% | 42.9% | 0% |
| double no-single-braid | normal | 71 | 63.4% | 21.1% | 52.1% | 11.3% |
| double no-single-braid | braid | 0 | — | — | — | — |

Singles overall (all types) stay in the Additive-BOM reuse band: ~53–58%
moved later, mean moves ~0.9–1.6, never-moved singles that touched control
= **0** in every cell. Single-normals that do get drafted have a positional
role (40–52% touch control; 11–20% swap when allowed). Single braids, on
the rare benches that keep one, are walked, not hoarded in-hand — the
hoard is the **16-tile reserve** at pick time.

Composition of placed singles in the full setup is mostly normal + zag +
bead; braids are 8–9 tiles in 100 games. Nobraid converts those 1-ring
heavies back into light-election singles, so more 1-normals reach the
bench (71 vs 45) and more of them sit on control.

## Qualitative log review

Citations are `log` / `pairing` / `game` / `turns`. Inner-ring node ids
are `{9,1,12,3,16,6,18,21}`; center is 0. Every first take in these 400
games is a MOVE. CLAIM-on-center is ~0.

### Link-once still sits

[`h2h_full_link_once.jsonl.gz`](h2h_full_link_once.jsonl.gz) / mcts vs
minimax / game 4 / 32 turns: P2 MOVEs onto 0 at t8 and never leaves; P1
never contests the origin; lockout. Same script as #82 IndependentMax,
just longer Additive games. Sit-and-win is 40% of takes.

The long exception is walking, not overwhelm:
[`h2h_full_link_once.jsonl.gz`](h2h_full_link_once.jsonl.gz) / mcts vs
minimax / game 1 / 152 turns. P2 takes t6; P1 pops from inner 12 at t13
and again at t21/t25; then from t46 both sides walk 2-normals on and off
0 at budget 0. One SWAP at t101 (1-normal 11↔12) is a self-reposition,
not a steal. Control-4 hold.

### Double-count: inner-adjacent trades, then zero-budget walking

A real contest:
[`h2h_full_double.jsonl.gz`](h2h_full_double.jsonl.gz) / mcts vs minimax /
game 8 / 94 turns. P2 takes 11→0 at t6 (3-normal). P1 pops from inner 21
at t9 (mutual with self); P2 retakes t10; P1 pops from 9 at t23; P2
retakes t24; P1 pops from 3 at t27; P1 takes t29; P2 pops from 18 at t30.
Four center trades in the first 30 turns, ratchet to 2. Then from t47 P1
sits on 0 while P2 walks a 2-normal around the inner ring at budget 0
for ~40 turns. P1 walks off and back at t91–94 for the hold. Trades first,
stalling later — Liam’s preferred “overwhelm is a trade” shows up, then
the hot-potato from #82.

A sit that double-count does **not** prevent:
[`h2h_full_double.jsonl.gz`](h2h_full_double.jsonl.gz) / mcts vs minimax /
game 2 / 36 turns. P2 takes 9→0 at t10 with a 2-normal and never leaves.
P1 never reaches an adjacent hitch. Lockout at control-5. This is still
~20% of takes.

Late first take, because the first landing dies:
[`h2h_full_double.jsonl.gz`](h2h_full_double.jsonl.gz) / mcts vs minimax /
game 13 / 39 turns. P2 MOVEs 23→0 at t14 and is captured on the landing
(self + P1 on 18); survey does not count it. Surviving take is t28.
Five double-count games never record a surviving take.

### SWAP as a center steal — rare, and it works when it fires

[`h2h_full_double.jsonl.gz`](h2h_full_double.jsonl.gz) / minimax vs mcts /
game 16 / 112 turns. P1 takes t19; P2 pops from inner 6 at t20 (mutual);
P1 retakes t25; **P2 SWAP 21→0 at t30** (1-normal exchanges with P1’s
2-normal on the origin — a dislodge without a capture); SWAP 0→9 at t36
drops a 3-zag onto 0; SWAP 9→0 at t40 takes it back. Later SWAP 0→1 at
t90 with a 1-bead partner. This is the grammar the swap rule was for.
It happened in 8/100 double-count games (18 SWAPs, 5 dislodging).

Same steal under link-once:
[`h2h_full_link_once.jsonl.gz`](h2h_full_link_once.jsonl.gz) / mcts vs
minimax / game 23 / 57 turns. P1 sits from t5; P2 SWAP 6→0 at t36
(budget 0), then SWAP 0→9 at t46 leaving a 2-normal on the origin.

No-swap ablation still contests the center (never-left 26%). Inner-adjacent
MOVE remains the recapture grammar; SWAP is a spice, not the meal.

### Single braids are reserved away, then spent if they make the bench

Opening benches in the full-setup logs keep a single braid on 10/200
sides. Those 5 games *do* use them:

[`h2h_full_double.jsonl.gz`](h2h_full_double.jsonl.gz) / minimax vs mcts /
game 39 / 59 turns. P1 ADDs a 1-braid on spawn 2 at t1, walks 2→12→0 at
t9 (first take with a single braid), P2 pops it from inner 9 at t16.
P2’s matching 1-braid ADDs on 14 at t14, walks to 1, self-pops. Not
hoarded in-hand.

[`h2h_full_link_once.jsonl.gz`](h2h_full_link_once.jsonl.gz) / mcts vs
minimax / game 44 / 25 turns. Both 1-braids are capture bombs on the
outer/inner ring (t15 pops three, t20 pops four), then a 3-braid takes
center at t24 and is immediately popped. Lockout. Exact +1 is not the
story; Additive mismatch stacks are.

[`h2h_full_link_once.jsonl.gz`](h2h_full_link_once.jsonl.gz) / minimax vs
mcts / game 37 / 66 turns is the single-heavy bench (n1=5, one 1-braid).
P2’s 1-braid ADDs on 17 at t14 and walks the outer ring; P1’s 1-braid
ADDs at t46 (budget 0) and shuffles 23→5→2→8→11 for 20 turns. Neither
sits on control as a braid. P1 wins by elimination.

Players do not hoard single braids during the game. The search picker
hoards them in the unused 16-tile bag (`combo_score` still calls
`braid_fulfilled_by`, which is coverage / min 2 slots, so a 1-slot exact
match is invisible at pick time; 1-rings also lose on raw weight).

### What the non-holder does

When the origin is contested, the answer is still **MOVE onto an inner
neighbor** and hope Additive hitch + braid-only match pops the holder
(usually a mutual trade). SWAP onto the occupied center is the new line
and is legal only with a 1-normal adjacent to 0. Vacant-center SWAP is
illegal by rule, and no first take is a SWAP. The non-holder’s inner
captures stay below the holder’s (0.77–0.96 vs 1.27–2.00). Zero-budget
share is 24–30%: late games are walking.

Lockouts (26–35%) still look like “holder occupied enough of the 9-node
set that the opponent ran out of legal ADD/MOVE/SWAP,” not “can’t
overwhelm the center piece.”

## Open design questions

- **Link-once vs double-count.** Counting the center petal once is the
  clean unique-pair rule. Stacked with this recipe it *undid* the #82
  Additive-BOM contest (never-left 39% → 44%). Restoring two arcs is the
  cell that actually drops sit-and-win to 20%. Should the live default
  stay per-arc, use link-once everywhere, or weight center↔inner by
  something other than 1 or 2?
- **Single braids never reached play.** 18.7% of generated tiles, 0.5% of
  benches, 5 games. `--pick search` plus coverage-only `braid_fulfilled_by`
  in `combo_score` dumps 1-rings into reserves. Human drafting would see
  more. Should pick honor `braid-only-exact` (1-slot +1), give 1-rings a
  pick bonus, or force a single onto the 11?
- **5:3 coin + single-braids = 27% braid tiles.** The ~10% target only
  holds with `--single-braids off` (8.3%). If singles stay on, retune the
  heavy coin? A separate 1-ring heavy rate?
- **Exact extras-allowed.** Current rule is `need[c] > 0 ⇒ have[c] ==
  need[c]`; extra unused colors on the neighbor are OK. Is that “exact,”
  or should the multisets be equal (no extras)?
- **SWAP is spice.** 0.08–0.18/game, 5 dislodging steals in 100
  double-count games. When it fires it is the intended steal (game 16).
  No-swap first-taker win rate is actually closer to 50%. Keep
  `normal-only`, revert to `on`, or drop SWAP until pick feeds 1-normals?
- **Nobraid ≥ full setup.** Exact 2-slot matching + double-count +
  Additive braid-only scoring is doing the counterplay. Single braids are
  not. Drop `--single-braids` and keep `braid-only-exact`?
- **Lockouts 26–35%.** Still the cost of forcing a reaction. Acceptable?
- **Hot-potato vs overwhelm.** Contested double-count games still spend
  their second half walking at budget 0 (game 8 t47–t90). Liam preferred
  trades to stasis; this package produces more trades *and* more walking.
- **First-taker win rate.** Never-left 23% does not mean the non-holder
  wins: full double first-taker 56%, nobraid 51%. Goal “center changes
  hands” or “the second player can win”?
- **Minimax-as-P1.** Still 33–36/50 under the full setup. Shallow
  counterplay search may over-value the sit.
- **CLAIM** is still dead (~0.02/game, ~0 on center). Inner-adjacent MOVE
  (and rare SWAP) is the only recapture grammar.
- **Live defaults.** `--single-braids` off, `--braid-scoring current`,
  `--link-once` off, `--singles-swap on`. Should any of those flip for
  play? This package’s best-looking *bot* cell is double-count +
  `braid-only-exact` **without** single braids, swap optional.
