# Focused MCTS (deeper trees, less root spread)

Stacked on PR #63 (`cursor/search-pruning-f5e4`). **Do not merge.** Independent
MCTS knobs plus a `deep_mcts` personality that tries to spend the same work
budget deeper, instead of spreading visits across 55–88 shallow legal moves.

Quote numbers from the committed logs in this directory. Do not overwrite;
append a new file if a rerun is needed.

## Hypothesis

With ~55–88 legal moves, MCTS spends extra budget on many near-equal root
moves, so more iterations do not buy depth or strength. Tested first with
per-move root-spread logging on live `enhanced` MCTS, then with one knob at
a time.

Root-spread (printed as `# root_spread` on every bench log):

| Field | Meaning |
|---|---|
| entropy | Shannon entropy of root visits (nats) |
| eff_branch | `exp(entropy)` — 1 if all visits on one move, ~legal if uniform |
| top1 | visit share of the most-visited child |
| near_q | children whose Q is within `1/sqrt(n_best)` of the best Q |
| q_spread10 | max Q − min Q among the 10 most-visited children |
| stability | fraction of iteration checkpoints whose best move matches the final pick |
| legal | real legal moves at the root |

Visit-weighted MCTS depth still excludes the root. Factored ADD plies do not
count as depth.

## Package

Release `apa-kan-bench`, zags on, doubles allowed (`SamePieceTwice::Off`),
shared-channel beads with 2–3 groups (live default), `--pick search`,
`--jobs 4`, `--no-depth-probe`. Never doubles-disallowed.

Work budgets: calibrated `nodes:132000,iters:2600` and 4× `iters:10400`.

## Knobs (`SearchConfig` / `--search k=v`)

Each lever is independently toggleable so it can feed a bot personality.

| Knob | Spec key | What it does |
|---|---|---|
| Move priors (PUCT) | `priors`, `prior-temp` | softmax of `cheap_order` (captures, center, killers/history) as the PUCT prior instead of eval-of-child |
| Prior-ordered expansion | `pw-order` | expand untried in prior/order-score order (highest first) |
| Progressive widening | `pw`, `pw-a`, `pw-alpha` | cap children at `1 + pw_a * visits^pw_alpha` |
| Tree reuse | `reuse` | keep the subtree under the move played (and the opponent reply). Default **on** |
| Transposition-aware stats | `mcts-tt` | share visits/value by Zobrist hash (tree stays a tree; backup skips cycles) |
| Factored ADDs | `factored` | virtual choose-tile ply, then destination |
| Cut-off leaf search | `leaf` | 1–2 ply capture-first minimax at the playout leaf instead of static eval |
| MCTS-solver | `solver` | prove wins/losses (lockout, elimination, other terminals); a parent is a win as soon as any child is |

`--search-profile deep_mcts` (aliases `deep`, `focused`) is the personality.
Example TOML: `apa-kan/game/bots/deep_mcts.toml`. Overlay with `--search` /
`--p1-search` / `--p2-search`.

### Live `deep_mcts` (after H2H)

`enhanced` + move-ordering softmax priors + prior-ordered expansion + tree
reuse + solver. **Progressive widening stays off** (same as `enhanced`).
PUCT stays 1.6. Factored ADDs, 1-ply leaf search, and MCTS transpositions
are implemented but **off** in the preset.

The first H2H used an earlier combo (prior-ordered PW + PUCT 0.9). That
searched deeper and lost 4-2-26 vs `enhanced`; the preset was then locked
to priors without extra widening.

## Ablation (`ablate.txt`)

Release, `nodes:132000,iters:2600`, jobs=1. Same openings as
`search_pruning/ablate.txt` (seeds 1, 2, 3 and 6 greedy plies from seed 4).
MCTS-only extra rows skip minimax. Elapsed 140.2 s.

The `deep_mcts` rows in this file are the **pre-retune** combo (priors + PW
+ factored ADDs + leaf=1 + MCTS TT + solver). Live `deep_mcts` matches
`enhanced` plus priors/pw-order/solver, not those rows.

**open1 (88 legal)**

| profile | avg / max depth | entropy | eff_branch | top1 | stability |
|---|---|---|---|---|---|
| enhanced | 2.26 / 6 | 2.19 | 8.9 | 0.41 | 0.88 |
| enhanced+priors | **2.60 / 6** | 1.96 | **7.1** | **0.59** | 0.62 |
| enhanced+prior_pw | **3.77 / 12** | 2.11 | 8.2 | 0.28 | 0.50 |
| enhanced+noreuse | 2.26 / 6 | 2.19 | 8.9 | 0.41 | 0.88 |
| enhanced+mcts_tt | 2.17 / 6 | 2.31 | 10.1 | 0.36 | 0.88 |
| enhanced+factored | 1.99 / 6 | 2.06 | 7.9 | 0.50 | 0.38 |
| enhanced+leaf1 | 1.71 / 3 | 3.23 | **25.2** | 0.07 | 0.38 |
| enhanced+solver | 2.26 / 6 | 2.19 | 8.9 | 0.41 | 0.88 |
| deep_mcts (old combo) | 2.82 / 11 | 2.87 | 17.7 | 0.16 | 0.12 |

**mid6 (70 legal)**

| profile | avg / max depth | entropy | eff_branch | top1 | stability |
|---|---|---|---|---|---|
| enhanced | 2.69 / 6 | 1.47 | 4.3 | 0.75 | 0.88 |
| enhanced+priors | 2.92 / 8 | 0.78 | 2.2 | 0.89 | 1.00 |
| enhanced+prior_pw | **4.52 / 15** | 0.61 | 1.8 | 0.90 | 1.00 |
| enhanced+leaf1 | 1.71 / 3 | 3.34 | 28.3 | 0.06 | 0.88 |
| deep_mcts (old combo) | 3.22 / 12 | 1.57 | 4.8 | 0.37 | 0.62 |

Hypothesis is **partly confirmed**. `enhanced` on 88 legal is not uniform
(eff_branch ~9, not 88) but is still shallow (avg 2.26, max 6). Priors
concentrate visits. Prior-ordered PW is the depth lever. Factored ADDs and
1-ply leaf search spread visits and shallow the tree. MCTS TT slightly
hurts. Solver and reuse are no-ops on a single frozen position (reuse
matters across moves; solver needs terminals in the tree).

`enhanced+priors` in this ablation also set PUCT 0.9; live `deep_mcts` keeps
PUCT 1.6.

## Strength H2H (calibrated budget)

Mixed decks, both seats, `nodes:132000,iters:2600`, `--jobs 4`.

### Prior-PW combo vs enhanced MCTS (discarded)

`h2h_mcts_deep_vs_enhanced.txt` / `h2h_mcts_enhanced_vs_deep.txt`.
Prior-ordered PW + PUCT 0.9.

| Log | P1 | P2 | P1 W-D-L | mcts_avg / max | lockouts |
|---|---|---|---|---|---|
| `h2h_mcts_deep_vs_enhanced` | prior-PW | enhanced | **2-1-13** | 3.20 / 22 | 1 |
| `h2h_mcts_enhanced_vs_deep` | enhanced | prior-PW | **13-1-2** | 3.12 / 34 | 0 |

Combined prior-PW is **4-2-26**. Deeper (avg ~3.1 vs ~2.3) and weaker.

### Live `deep_mcts` vs enhanced MCTS

`h2h_mcts_live_deep_vs_enhanced.txt` / `h2h_mcts_live_enhanced_vs_deep.txt`.
12 games/seat. Overlay check at 8/seat (`h2h_mcts_priors_vs_enhanced`,
`h2h_mcts_enhanced_vs_priors`) was 7-3-6 for priors.

| Log | P1 | P2 | P1 W-D-L | mcts_avg / max | `# root_spread` |
|---|---|---|---|---|---|
| `h2h_mcts_live_deep_vs_enhanced` | deep_mcts | enhanced | **10-0-2** | 2.33 / 11 | entropy=1.94 eff_branch=11.6 top1=0.47 |
| `h2h_mcts_live_enhanced_vs_deep` | enhanced | deep_mcts | 7-1-4 | 2.31 / 11 | entropy=1.92 eff_branch=11.8 top1=0.47 |

Combined live `deep_mcts` is **14-1-9** vs enhanced MCTS (24 games). Depth
is not the prior-PW spike; concentration is similar to enhanced over a full
game (legal ~44–46).

End reasons, live deep as P1: elim=2 locked_out=**5** control=1 low_value=4.
Enhanced as P1: locked_out=3 control=1 low_value=8. Lockouts are 26–93
turns on mirrored mixed benches, not a four-move script. Not called a
balance-breaking exploit. One short win: seed 3, 20 turns, P2 low_value.

`add@14` as P1 first spawn is 4-0-0 (n=4) in the live-deep-as-P1 log.

### Live `deep_mcts` vs enhanced minimax

`h2h_mcts_deep_vs_mm.txt` / `h2h_mm_enhanced_vs_mcts_deep.txt`. 12/seat.

| Log | P1 | P2 | P1 W-D-L | mm_avg / max | mcts_avg / max |
|---|---|---|---|---|---|
| `h2h_mcts_deep_vs_mm` | deep_mcts MCTS | enhanced minimax | 4-2-6 | 4.40 / 16 | 2.27 / 9 |
| `h2h_mm_enhanced_vs_mcts_deep` | enhanced minimax | deep_mcts MCTS | 6-0-6 | 3.94 / 16 | 2.20 / 10 |

Combined deep_mcts vs enhanced minimax is **10-2-12**. Roughly even, slight
minimax edge. One 18-turn P2 low_value (`g5` seed 5) and one 196-turn P2
low_value (`g12` seed 12). No lockout sweep.

## Long MCTS (4×, `mcts_long.txt`)

Same config as `search_pruning/mcts_long.txt`: `--pairings mcts:mcts`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`,
`--games 6`, `--budget nodes:132000,iters:10400`, `--search-profile deep_mcts`.
Elapsed **3460.8 s**.

```
# seat_totals  P1 11-7-12  P2 12-7-11  games=30
# end_reasons  elim=0 locked_out=1 control=3 low_value=23 turn_limit=3
# search_depth  mcts_avg=3.14 mcts_max=19
# root_spread  entropy=1.36 eff_branch=5.5 top1=0.62 stability=0.82 legal=42.0 n=1837
# matchup_totals  mixed vs mixed 4-1-1  |  braid vs braid 1-3-2  |  bead vs bead 3-1-2  |  braid vs bead 1-1-4  |  bead vs braid 2-1-3
# archetype_seat  mixed P1=4-1-1 P2=1-1-4  |  braid P1=2-4-6 P2=5-4-3  |  bead P1=5-2-5 P2=6-2-4
# short_games  turns_le_20=5 lockout=0 control=0
```

| Matchup | P1 W-D-L | avg turns | vs enhanced 4× (`search_pruning/mcts_long`) |
|---|---|---|---|
| mixed vs mixed | **4-1-1** | 67.33 | was **0-0-6** P2 sweep |
| braid vs braid | 1-3-2 | 24.33 | 2-1-3 |
| bead vs bead | 3-1-2 | 89.83 | 1-1-4 |
| braid vs bead | 1-1-4 | 55.67 | 1-0-5 |
| bead vs braid | 2-1-3 | 69.00 | 4-0-2 |

- **Seat:** P1 11-7-12 / P2 12-7-11. The enhanced 4× **P2 20-2-8** lean and mixed P2 6-0-0 sweep **did not replicate**.
- **Deck:** bead vs braid directed is bead **6-2-4** (bead as P1 2-1-3, bead as P2 4-1-1 from `braid vs bead`). Not the earlier 9-0-3. Extra 4× sample (`mcts_long_extra.txt`) is bead **12-1-11** over braid; combined **18-3-15**.
- **Depth:** avg **3.14**, max **19** vs enhanced 2.48 / 13. Root visits more concentrated (eff_branch 5.5, top1 0.62).
- **Endings:** 23 low_value, 3 control, 3 turn-limit, 1 lockout (`braid vs bead` g3 seed=300003, 34 turns).
- **Openings:** every first action is a spawn ADD. No four-move line repeated. `add@5` is 1-4-3 (n=8, decided win rate 0.25).
- **Braid mirror shorts:** seeds 100002 / 100003 / 100004 are 9 / 11 / 13-turn low_value **draws**. Seed 100004 is the same 13-turn braid-mirror example from #63; not a new winning exploit.

### Extra 4× mixed + bead-braid (`mcts_long_extra.txt`)

Seed 100, 12 games/matchup, same 4× budget. Elapsed **2598.9 s**.

```
# seat_totals  P1 19-1-16  P2 16-1-19  games=36
# matchup_totals  mixed vs mixed 5-0-7  |  braid vs bead 7-0-5  |  bead vs braid 7-1-4
# search_depth  mcts_avg=2.97 mcts_max=17
# short_games  turns_le_20=0
```

| Matchup | extra n=12 | plus `mcts_long` n=6 | combined |
|---|---|---|---|
| mixed vs mixed | 5-0-7 | 4-1-1 | **9-1-8** |
| braid vs bead | 7-0-5 | 1-1-4 | 8-1-9 |
| bead vs braid | 7-1-4 | 2-1-3 | 9-2-7 |

Combined mixed (18 games) is even. Combined bead vs braid (36 directed) is bead **18-3-15**. No P2 6-0-0 sweep, no 9-0-3 deck exploit. `add@23` is 4-0-0 (n=4) in this extra file only; not a repeated four-move line. Four lockouts at 27–111 turns, no short-game lockout script.

## Correctness

- `threads_one_is_deterministic_with_deep_mcts`
- `solver_picks_immediate_center_hold_win` (center-hold, `center_hold_n=1`)
- `tree_reuse_keeps_the_played_subtree` / `tree_reuse_off_rebuilds`
- `root_spread_is_populated`
- `SearchConfig` profile/TOML/spec round-trips including the new keys

## Design questions

1. **Cheap-order priors vs learned priors.** Capture/center ordering as a
   softmax prior concentrates visits, but prior-ordered PW + low PUCT over-
   trusts that heuristic and lost 4-2-26. A policy net or eval-based prior
   might keep the depth of PW without the strength collapse.
2. **Factored ADDs.** Implemented; ablation shallowed the tree (open1 1.99/6
   vs 2.26/6). The virtual ply may be spending visits on tile choice that
   should go to destinations. Worth a different backup / one-ply expand.
3. **MCTS transpositions.** Stats sharing by hash (not a DAG) slightly
   increased effective branching. Backup skips in-path hashes. Cycle-safe,
   not clearly useful at this budget.
4. **1-ply leaf search.** Turned cut-off playouts into near-uniform Q
   (top1 0.07, eff_branch 25). Too noisy relative to static eval.
5. **Solver.** Correct on a 1-ply hold-win; no-op on opening ablations.
   How often it fires in real games is still open (needs a per-move proven
   count in the log).
6. **Lockout rate** with live `deep_mcts` as P1 (5/12 mixed at 2600 iters).
   The 4× self-play package had only 1 lockout / 30, so the H2H spike looks
   matchup-specific, not a lockout script.
7. **Mixed P2 lean and bead>braid** from PR #63's 4× enhanced MCTS. This
   preset's 4× self-play is even by seat (11-7-12 / 12-7-11). Combined with
   the extra sample, mixed is 9-1-8 and bead vs braid is 18-3-15. Those
   earlier leans look engine-specific to `enhanced` at 4×, not a rule
   exploit.
