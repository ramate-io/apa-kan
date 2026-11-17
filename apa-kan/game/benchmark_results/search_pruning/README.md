# Search pruning / deeper play

Stacked on PR #62 (`cursor/bead-shared-channel-1647`). Makes minimax and MCTS
search deeper inside the **same** work budgets, then reruns the #62 archetype
matchups with the stronger engines.

Quote numbers from the committed logs in this directory. Do not overwrite;
append a new file if a rerun is needed.

## Profiles

| Profile | Minimax | MCTS |
|---|---|---|
| `plain` | alpha-beta, no TT/ordering/LMR/PVS/qsearch | uniform random cut-off playouts, no PW |
| `classic` | PR #62: TT + ordering + quiescence | heuristic cut-off playouts, no PW, 1 thread |
| `enhanced` (default) | classic + LMR (fail-high re-search, skip root) + PVS | classic + progressive widening |

`--search k=on\|off` overlays flags. `--search-threads N` runs N independent
MCTS trees at the root; worker `t` is seeded with `rng.gen() XOR (t+1)*golden`.
`threads=1` draws nothing extra and is bit-identical. Keep `threads=1` with
`--jobs` so game-level parallelism stays deterministic.

`--ablate-search` writes one row per lever × opening/midgame position.
`--ply-probe N` times a handful of positions against the work/time limit and
reports completed ID depth vs the target.

## Package

Release `apa-kan-bench`, zags on, doubles allowed, shared-channel beads with
2–3 groups (live default), `--pick search`, `--jobs 4`, `--no-depth-probe`,
`--pairings minimax:mcts,mcts:minimax`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`.

`# search_depth` is minimax completed iterative-deepening depth (avg/max,
move-weighted) and MCTS visit-weighted tree depth excluding the root.

## 10-ply probe (committed)

`--ply-probe 10 --p1-deck braid --p2-deck bead`, enhanced unless noted.
10 ply was **not reached** on any position. Midgame `braid_vs_bead_ply24` is
24 greedy plies from a braid vs bead opening.

| Log | Limit | open | open2 | braid_vs_bead_ply24 |
|---|---|---|---|---|
| `ply10_calibrated.txt` | 132000 nodes | 3 / 1951 ms | 4 / 2109 ms | 4 / 867 ms |
| `ply10_4x.txt` | 528000 nodes | 4 / 7134 ms | 4 / 4353 ms | 5 / 4007 ms |
| `ply10_20s.txt` | 20 s wall | 5 / 1.62M nodes | 6 / 2.02M nodes | 6 / 3.31M nodes |
| `ply10_20s_noq.txt` | 20 s, qsearch off | 6 / 6.09M nodes | 6 / 5.80M nodes | 8 / 4.64M nodes |

At ~55 legal moves, 10-ply ID is not feasible at the calibrated budget.
20 s without quiescence reached 8 ply on the narrower midgame (54 legal).

## Full-game package

Same pairings/matchups as #62. Quote `# search_depth` and W-D-L from the
committed log files.

### Depth vs PR #62 classic (`groups_shallow`)

| Log | Budget | Games | mm_avg / mm_max | mcts_avg / mcts_max | P1 W-D-L | elapsed |
|---|---|---|---|---|---|---|
| #62 `groups_shallow` (classic) | 132000 / 2600 | 50 | 4.25 / 16 | 1.96 / 7 | 20-6-24 | 1495.9 s |
| `enhanced_shallow` | 132000 / 2600 | 50 | **4.30 / 16** | **2.31 / 10** | 29-0-21 | 744.4 s |
| #62 `groups_deep` (classic) | 528000 / 10400 | 10 | 4.49 / 7 | 2.32 / 9 | 3-0-7 | 557.0 s |
| `enhanced_deep` | 528000 / 10400 | 10 | **5.53 / 16** | **2.58 / 11** | 3-0-7 | 2788.0 s |

At the calibrated budget, minimax ID ply is essentially unchanged
(quiescence still eats leaf nodes). At 4×, enhanced ID rises to **5.53**
avg (classic 4.49) and max 16. MCTS trees are deeper with progressive
widening (+0.35 avg / +3 max shallow; +0.26 avg / +2 max at 4×).

### Balance (`enhanced_shallow`)

```
# matchup_totals  mixed vs mixed 8-0-2  |  braid vs braid 7-0-3  |  bead vs bead 5-0-5  |  braid vs bead 4-0-6  |  bead vs braid 5-0-5
# archetype_seat  mixed P1=8-0-2 P2=2-0-8  |  braid P1=11-0-9 P2=8-0-12  |  bead P1=10-0-10 P2=11-0-9
# short_games  turns_le_20=1 lockout=0 control=0
```

No archetype wins from both seats against the other. Mixed's 8-0-2 as P1
is a **seat** lean (mixed as P2 is 2-0-8). Bead vs braid is 5-0-5 either
direction of the named matchup totals above. One short win:
`minimax vs mcts [braid vs braid] g4 seed=100004` low_value at 13 turns
(mirrored braid benches) — not a deck exploit.

### `enhanced_deep` (4×, committed)

```
# search_depth  mm_avg=5.53 mm_max=16  mcts_avg=2.58 mcts_max=11
# matchup_totals  mixed 1-0-1  |  braid 1-0-1  |  bead 1-0-1  |  braid vs bead 0-0-2  |  bead vs braid 0-0-2
# archetype_seat  mixed P1=1-0-1 P2=1-0-1  |  braid P1=1-0-3 P2=3-0-1  |  bead P1=1-0-3 P2=3-0-1
```

Same P1 3-0-7 as #62 classic 4×. Cross braid/bead is a P2 sweep (4 games),
seat not deck. One long mixed game (`mcts vs minimax` mixed, 191 turns,
P1 low_value) dominated wall-clock; 10-ply was still not reached
(`mm_avg=5.53`).

4× braid-vs-bead handful (the 10-ply target; time/move from `ply10_4x.txt`
is 4–7 s at 4–5 ply, not 10):

```
minimax vs mcts  braid vs bead  P1  0-0-1  104.00  mm avg 6.04 max 9
mcts vs minimax  braid vs bead  P1  0-0-1  30.00   mm avg 4.07 max 5
minimax vs mcts  bead vs braid  P1  0-0-1  38.00   mm avg 6.32 max 10
mcts vs minimax  bead vs braid  P1  0-0-1  24.00   mm avg 5.50 max 6
```

### Strength: enhanced vs classic (mixed, 6 games/seat)

| Log | P1 profile | P2 profile | P1 W-D-L |
|---|---|---|---|
| `h2h_mm_enhanced_vs_classic` | enhanced minimax | classic minimax | **4-1-1** |
| `h2h_mm_classic_vs_enhanced` | classic minimax | enhanced minimax | 2-1-3 |
| `h2h_mcts_enhanced_vs_classic` | enhanced MCTS | classic MCTS | 1-1-4 |
| `h2h_mcts_classic_vs_enhanced` | classic MCTS | enhanced MCTS | 2-1-3 |

Minimax enhanced is **7-2-3** across 12 mixed games (both seats).
MCTS enhanced is **4-2-6** — progressive widening deepens the tree but
did not win this sample. Keep classic MCTS available behind the flag.

### Suspicious lines

MCTS typically opens with ADDs; nothing in the committed JSONL looks like
a unique forced opening. The 13-turn braid mirror (`seed=100004`) was
re-probed at 4× (`verify_short_braid_100004.txt`): opening ID is still 4
ply / 8591 ms with 88 legal moves, so it is not a solved force.

**Main question:** no archetype or line wins regardless of seat and
opponent. Closest leans are P1 mixed (seat) and P2 in 4× cross games
(also seat).
