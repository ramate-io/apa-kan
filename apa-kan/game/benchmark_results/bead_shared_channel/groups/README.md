# Shared-channel default with 2–3 bead groups

Rerun of the [`../`](../) package under the **live default**: rule B
(shared-channel bead scoring) and piece-level **2–3 bead groups** (no
single-bead tiles). Doubles allowed, zags on. Do not overwrite; quote numbers
from the committed logs in this directory.

Compare against the previous rule-B runs in [`../shared_shallow`](../shared_shallow.txt)
and [`../shared_deep`](../shared_deep.txt), which still generated 1-bead tiles.

## Package

Release `apa-kan-bench`, `--pick search`, `--jobs 4`, `--no-depth-probe`,
`--no-same-piece-twice off`, `--pairings minimax:mcts,mcts:minimax`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`.
Bead scoring is the default (`shared`); independent remains
`--bead-scoring independent`.

| File | Budget | Games | elapsed |
|---|---|---|---|
| `groups_shallow` | `nodes:132000,iters:2600` (calibrated) | 50 = 5 matchups × 2 seats × 5 seeds | 1495.9 s |
| `groups_deep` | `nodes:528000,iters:10400` (4× calibrated) | 10 = 5 matchups × 2 seats × 1 seed | 557.0 s |

Bead-heavy quota: ≥10 bead rings, ≥2 tiles with 2+ bead rings, ≥1 3-bead
tile, and no 1-bead tiles. JSONL collections in these logs have **0
single-bead tiles** (873 grouped rings on shallow decks, 166 on deep).

`# search_depth` is minimax completed iterative-deepening depth (avg/max,
move-weighted) and MCTS visit-weighted tree depth excluding the root.

## Results: still no dominant deck strategy

Dropping singles and padding 2–3 bead groups does **not** create an
unbeatable archetype. `braid vs braid` is identical to the previous
rule-B logs (no beads in those collections). The shallow cross matchup
leans **bead**, but bead as P1 vs braid is only 4-2-4, and at 4× the
four cross games are a **P2 sweep** (2 bead, 2 braid) — seat, not deck.

| Log | P1 W-D-L | Mean turns | Ends (lock/ctrl/LV/TL) | Opp captures | bead contrib / needed / spawn2ring | elapsed |
|---|---|---|---|---|---|---|
| `shared_shallow` (old, with singles) | **25-3-22** | 50.58 | 15 / 8 / 24 / 3 | 6.54 (327) | 288 / 107 / 2 | 1175.3 s |
| `groups_shallow` | **20-6-24** | 61.48 | 7 / 12 / 25 / 5 | 6.04 (302) | 196 / 136 / 0 | 1495.9 s |
| `shared_deep` (old, with singles) | **6-0-4** | 36.00 | 1 / 1 / 8 / 0 | 6.90 (69) | 61 / 35 / 2 | 721.3 s |
| `groups_deep` | **3-0-7** | 29.10 | 3 / 2 / 5 / 0 | 6.00 (60) | 32 / 27 / 0 | 557.0 s |

`braid vs braid` control (quoted both shallow logs):

```
minimax vs mcts  braid vs braid  P1  4-0-1  34.00
mcts vs minimax  braid vs braid  P1  1-1-3  73.80
```

### Archetype records

**Groups shallow:**

```
# archetype_seat  mixed P1=4-1-5 P2=5-1-4  |  braid P1=6-3-11 P2=8-3-9  |  bead P1=10-2-8 P2=11-2-7
# matchup_totals  mixed vs mixed 4-1-5  |  braid vs braid 5-1-4  |  bead vs bead 6-0-4  |  braid vs bead 1-2-7  |  bead vs braid 4-2-4
# short_games  turns_le_20=0 lockout=0 control=0
```

- mixed 9-2-9, braid 14-6-20 (41%), bead 21-4-15 (58%)
- braid vs bead combined (braid W-D-L, any seat): **5-4-11**
- Bead as P2 won 7 of 8 decided `braid vs bead` games, but bead as P1 vs
  braid is 4-2-4. Not a lock from both seats.
- No win in ≤20 turns.

Previous rule-B shallow cross was **10-1-9** braid (even). Removing
singles flipped that lean toward stacked beads; it did not produce a
forced win.

**Groups deep 4×:**

```
# archetype_seat  mixed P1=0-0-2 P2=2-0-0  |  braid P1=1-0-3 P2=3-0-1  |  bead P1=2-0-2 P2=2-0-2
# matchup_totals  mixed vs mixed 0-0-2  |  braid vs braid 1-0-1  |  bead vs bead 2-0-0  |  braid vs bead 0-0-2  |  bead vs braid 0-0-2
# short_games  turns_le_20=0 lockout=0 control=0
```

- braid vs bead combined: **2-0-2** — even; P2 won all four cross games
- `bead vs bead` 2-0-0 is both seats, **identical benches**, 21 turns —
  seat/engine, not stacking
- mixed also P2-swept (0-0-2). Overall P2 **7-0-3**. That is a seat
  lean at n=10, not an archetype lockout.

Previous rule-B deep cross was **1-0-3** braid (bead lean, n=4). Same
conclusion: not robust, not a dominant strategy.

### Engine

P1 W-D-L:

| Log | minimax vs mcts | mcts vs minimax |
|---|---|---|
| shared_shallow | 16-0-9 | 9-3-13 |
| groups_shallow | 11-3-11 | 9-3-13 |
| shared_deep | 4-0-1 | 2-0-3 |
| groups_deep | 2-0-3 | 1-0-4 |

The old minimax ≈2:1 edge shrinks once singles are gone. MCTS as P1 is
unchanged at calibrated depth (9-3-13). Short lockouts remain
same-bench games, not an archetype punching through a prepared answer.

### Search depth

Quoted `# search_depth` (move-weighted):

| Log | mm_avg | mm_max | mm_n | mcts_avg | mcts_max | mcts_n |
|---|---|---|---|---|---|---|
| groups_shallow | 4.25 | 16 | 1538 | 1.96 | 7 | 1537 |
| groups_deep | 4.49 | 7 | 146 | 2.32 | 9 | 145 |

4× budget adds about a quarter-ply of completed ID on average
(4.25 → 4.49) and a third of a ply of MCTS tree (1.96 → 2.32). The
shallow `mm_max=16` is a late low-branching position in a long game,
not deeper search at the smaller budget (deep games averaged 29 turns
vs 61, so they never reached those empty boards).

### Secondary

- **Spawn two-ring single-bead captures:** 0 / 0. Generation no longer
  emits a 1-bead neighbor.
- **Stalls:** groups_shallow 5 turn-limits, 0 games that both hit the
  limit and a visible repeat ≥3. Deep: 0 turn-limits.
- **Bead-needed captures:** 136/302 (45%) shallow, 27/60 (45%) deep.
  Previous rule B with singles was 107/327 (33%) shallow. Stacked
  groups fire less often (`contrib` 196 vs 288) but are more often
  decisive when they do.
- **Deal mix:** padding 2–3 groups raises 3-ring tiles
  (`ring_process` n3 101 → 1541; opening n3 9 → 110). That is the
  largest generation change relative to `shared_shallow`.

Default stays shared-channel with 2–3 bead groups.
