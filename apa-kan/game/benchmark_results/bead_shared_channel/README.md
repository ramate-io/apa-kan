# Bead scoring: independent vs shared channel

Compares two bead scoring rules on the same search, pick, and deck package.
Doubles allowed, zags on. Do not overwrite; quote numbers from the committed
logs in this directory.

## Rules

Both rules keep the same **matching** test: a bead of color `C` is matched iff
the neighbor has `C` among non-zag rings (presence, not coverage). A match is
still −1 at most once per edge. Zag and braid channels are unchanged.

| | Combine | 1 unmatched bead vs 2-ring hitch | 3 unmatched beads vs 1-ring |
|---|---|---|---|
| **A. Independent** (default, `--bead-scoring independent`) | normal + bead | hitch 1 + unmatched 1 → `(0, 2)` | hitch 1 + unmatched 3 → `(0, 4)` |
| **B. Shared channel** (`--bead-scoring shared`) | `max(normal, bead)` per match and per mismatch | `max(1, 1)` → `(0, 1)` | `max(1, 3)` → `(0, 3)` |

Rule B stops beads doubling with a normal hitch. Beads still count per unmatched
ring when that count exceeds the hitch, so they remain a bomb against
non-matching braids (failed-braid +1 still stacks).

## Deck archetypes

Collections are 27 tiles; the 7+4 bench still uses `--pick search` (heuristic
rank + hill-climb + short rollouts under the same work budget). Generation is
the usual ring/color/zag/braid/bead process with biased numerators, then a
quota retry so the picker actually sees the intended tiles. Zag chance stays
at 1/7 (zags on).

| Flag | Sampling | Quota on the 27-tile collection |
|---|---|---|
| `mixed` | `braid_numer=1`, `bead_numer=1` (defaults) | none — the usual noisy deal |
| `braid` | `braid_numer=5`, `bead_numer=0` | ≥10 braid rings and ≥2 multi-braid (2–3 slot) tiles. Leftover slots after a 2-braid stay normal → braid+normal packing. |
| `bead` | `bead_numer=5`, `braid_numer=0` | ≥10 bead rings, ≥2 single-bead tiles, ≥1 tile with 2–3 beads. |

When the two sides have different archetypes the collections are generated
independently even if `identical_bags` is on. Same-archetype matchups still
mirror (then pick with knowledge of the opponent's 7).

Matchups in this package: `mixed vs mixed`, `braid vs braid`, `bead vs bead`,
`braid vs bead`, `bead vs braid` (the last two are seat-swapped mirrors of
each other).

## Package

Release `apa-kan-bench`, `--pick search`, `--jobs 4`, `--no-depth-probe`,
`--no-same-piece-twice off`, `--pairings minimax:mcts,mcts:minimax`.

| File | Rule | Budget | Games |
|---|---|---|---|
| `independent_shallow` | A | `nodes:132000,iters:2600` (calibrated) | 50 = 5 matchups × 2 seats × 5 seeds |
| `independent_deep` | A | `nodes:528000,iters:10400` (4× calibrated) | 10 = 5 matchups × 2 seats × 1 seed |
| `shared_shallow` | B | calibrated | 50 |
| `shared_deep` | B | 4× calibrated | 10 |

Deep eval is for **balance-breaking strategies**: an archetype, opening, or
line that gives one side an unbeatable or near-forced win regardless of seat
or opponent. Secondary: single-bead captures of a fresh 2-ring, stalls.

`# archetype_seat` in the text log is P1/P2 W-D-L for each archetype across
every game that archetype played. A dominant braid (or bead) strategy shows
up as a strong record from **both** seats, especially in the cross matchup.
`# short_games` counts wins in ≤20 turns (lockout/control) as a forced-win
hint; inspect those JSONL records for counterplay.

Numbers below are copied from the committed `*.txt` logs. Combined
seat-independent records are the sum of the two cross rows
(`braid vs bead` + `bead vs braid`).

## Results: no dominant deck strategy

Neither bead-stacking nor braid-packing gives one side an unbeatable or
near-forced path to winning. No archetype wins consistently from both seats
against both opponents. Deep search at 4× budget does not find an
unanswerable lockout line; the trailing archetype still has counterplay, and
the one shallow braid edge **reverses** at 4×. The stable gap is engine
(minimax vs MCTS), not deck.

| Log | P1 W-D-L | Mean turns | Ends (lock/ctrl/LV/TL) | Opp captures | bead contrib / needed / spawn2ring | elapsed |
|---|---|---|---|---|---|---|
| `independent_shallow` | **24-2-24** | 42.70 | 7 / 11 / 29 / 2 | 5.84 (292) | 270 / 103 / 0 | 952.4 s |
| `independent_deep` | **5-0-5** | 34.10 | 1 / 1 / 8 / 0 | 6.20 (62) | 59 / 30 / 3 | 637.3 s |
| `shared_shallow` | **25-3-22** | 50.58 | 15 / 8 / 24 / 3 | 6.54 (327) | 288 / 107 / 2 | 1175.3 s |
| `shared_deep` | **6-0-4** | 36.00 | 1 / 1 / 8 / 0 | 6.90 (69) | 61 / 35 / 2 | 721.3 s |

Sanity: `braid vs braid` is identical under A and B (no beads in those
collections). Quoted from both shallow logs:

```
minimax vs mcts  braid vs braid  P1  4-0-1  34.00
mcts vs minimax  braid vs braid  P1  1-1-3  73.80
```

### Archetype records (any seat)

Quoted `# archetype_seat`, then the combined W-D-L for that deck:

**Rule A independent, shallow (40 games with braid, 40 with bead, 20 mixed):**

```
# archetype_seat  mixed P1=5-0-5 P2=5-0-5  |  braid P1=11-2-7 P2=10-1-9  |  bead P1=8-0-12 P2=9-1-10
# matchup_totals  mixed vs mixed 5-0-5  |  braid vs braid 5-1-4  |  bead vs bead 4-0-6  |  braid vs bead 6-1-3  |  bead vs braid 4-0-6
```

- mixed 10-0-10 (50% decided)
- braid 21-3-16 (57%) — both seats win, neither dominates
- bead 17-1-22 (44%)
- braid vs bead combined (braid W-D-L, any seat): **12-1-7**

That 12-1-7 is the closest thing to an archetype edge, and it is not a lock:
bead still wins 7 of 19 decided cross games, including from both seats.

**Rule A independent, deep 4× (n=10):**

```
# archetype_seat  mixed P1=2-0-0 P2=0-0-2  |  braid P1=1-0-3 P2=2-0-2  |  bead P1=2-0-2 P2=3-0-1
# matchup_totals  mixed vs mixed 2-0-0  |  braid vs braid 1-0-1  |  bead vs bead 1-0-1  |  braid vs bead 0-0-2  |  bead vs braid 1-0-1
# short_games  turns_le_20=0 lockout=0 control=0
```

- braid vs bead combined: **1-0-3** — the shallow braid edge **flips**
- mixed P1 swept the two same-deck games; that is seat, not mixed-vs-other
- no win in ≤20 turns; one lockout (bead vs bead, 24 turns, same benches)

**Rule B shared-channel, shallow:**

```
# archetype_seat  mixed P1=5-1-4 P2=4-1-5  |  braid P1=10-1-9 P2=9-2-9  |  bead P1=10-1-9 P2=9-0-11
# matchup_totals  mixed vs mixed 5-1-4  |  braid vs braid 5-1-4  |  bead vs bead 6-0-4  |  braid vs bead 5-0-5  |  bead vs braid 4-1-5
```

- mixed 9-2-9, braid 19-3-18 (51%), bead 19-1-20 (49%)
- braid vs bead combined: **10-1-9** — even
- more lockouts than A (15 vs 7), concentrated in `bead vs bead` (6 of 10),
  but those are same-archetype games, 21–52 turns, split seats — not a
  one-sided lockout strategy

**Rule B shared-channel, deep 4×:**

```
# archetype_seat  mixed P1=1-0-1 P2=1-0-1  |  braid P1=2-0-2 P2=1-0-3  |  bead P1=3-0-1 P2=2-0-2
# matchup_totals  mixed vs mixed 1-0-1  |  braid vs braid 1-0-1  |  bead vs bead 1-0-1  |  braid vs bead 1-0-1  |  bead vs braid 2-0-0
# short_games  turns_le_20=1 lockout=0 control=0
```

- braid vs bead combined: **1-0-3** (same 3–1 bead lean as independent deep)
- `bead vs braid` 2-0-0 is two games; the mirror `braid vs bead` is 1-0-1
- one short win: `minimax vs mcts [bead vs bead] seed=200001` P1 low_value
  t=19, **identical benches** — engine, not a bead-stacking forced win

### Engine, not deck

Quoted pairing rows, P1 W-D-L:

| Log | minimax vs mcts (P1) | mcts vs minimax (P1) |
|---|---|---|
| independent_shallow | 16-1-8 | 8-1-16 |
| independent_deep | 3-0-2 | 2-0-3 |
| shared_shallow | 16-0-9 | 9-3-13 |
| shared_deep | 4-0-1 | 2-0-3 |

Minimax is ~2:1 vs MCTS at both budgets. Short lockouts (t≤20) are almost
always minimax beating MCTS with the **same** opening bench, not an
archetype punching through a prepared answer.

### Example games (from committed opening_deal / JSONL)

**1. Shallow braid “edge” that deep search answers (rule A).**
`independent_shallow` `# matchup_totals` `braid vs bead 6-1-3` /
`bead vs braid 4-0-6`. At 4×, `independent_deep`:

- `minimax vs mcts [braid vs bead] g1 seed=300001` P2 low_value t=28.
  P1 braid `n11 w15 d20`, P2 bead `n11 w15 d24`. Bead (MCTS) beats braid
  (**minimax**) by piece-count low-value. Bead has counterplay even against
  the stronger engine.
- `mcts vs minimax [braid vs bead] g1 seed=310001` P2 low_value t=50.
  Same matchup, minimax on bead wins a 50-turn game. Not a forced lockout.
- Mirror `braid vs bead` at 4× is 0-0-2: bead as P2 won **both** deep
  games. Combined cross 1-0-3 for braid — the opposite of dominance.

**2. Same-bench 18-turn lockout is engine, not bead stacking (rule A).**
`independent_shallow` `mcts vs minimax [bead vs bead] g2 seed=210002`
P2 locked_out t=18. Opening benches are identical
(`[oz+ge2-2o, ge+oz2-3^, pe1-1^, …]` both sides). JSONL: P2 MOVE captures
on turns 4 and 8 (cap=2 each) then locks on 18. MCTS as P1 with the same
bead-heavy 7+4 cannot hold. If bead-stacking were a forced win, P1 would
not fold in 18 with the identical deck.

**3. Bead as P1 vs braid, 20-turn lockout (rule B) — braid answers.**
`shared_shallow` `mcts vs minimax [bead vs braid] g4 seed=410004`
P2 locked_out t=20. P1 bead `n11 w13 d26`, P2 braid `n11 w17 d23`
(`[bd+bd2-3^, b1-2^, gz+b+t3-3o, od+od2-3^, …]`). Braid (minimax) locks
the bead MCTS seat. Same seed under rule A was also P2 in 28
(`independent_shallow` opening_deal). Bead-heavy vs braid-heavy is not a
one-way bomb.

**4. Shared-channel deep does not crown bead.**
`shared_deep` `bead vs braid 2-0-0` looks like a sweep until the mirror
`braid vs bead 1-0-1` is included (combined 3-1 bead, n=4). One of the
bead-as-P1 wins is a 41-turn **lockout of minimax** by MCTS
(`mcts vs minimax [bead vs braid] seed=410001`) — rare engine upset, not
a reproducible opening. The other is `minimax vs mcts [bead vs braid]
seed=400001` P1 low_value t=51. Shallow rule B on the same matchups was
10-1-9 braid.

### Secondary

- **Single-bead spawn captures of a 2-ring:** 0 / 3 / 2 / 2 across the four
  logs. Present under both rules at 4×; not a strategy, just a rare tactic.
  Rule B's unit test still forbids a lone unmatched bead from popping a
  fresh 2-ring (`shared_channel_single_bead_does_not_pop_a_two_ring`); the
  logged `spawn_two_ring_single_bead` counts can include other bead-needed
  captures on a spawn tile (e.g. stacked beads).
- **Stalls:** independent_shallow 2 turn-limits (one with a visible
  repeat ≥3); shared_shallow 3 turn-limits, 0 games that both hit the
  limit and a visible repeat ≥3. Deep logs: 0 turn-limits. No new stall
  pattern tied to an archetype.
- **Bead-needed captures:** ~35–48% of opponent captures still need the
  bead channel under both rules (A shallow 103/292, B shallow 107/327).
  Shared-channel does not remove beads as a capture tool; it only stops
  them stacking with a hitch.

Rule B is even across archetypes at calibrated depth and does not create a
dominant strategy at 4×. Rule A’s modest braid lean at calibrated depth is
not robust to more search. Default stays independent.
