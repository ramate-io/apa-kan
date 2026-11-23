# Capture-score deep search probe

Independent check of whether **base** capture-score stalling (4–4 at turn 200, inner-ring move ping-pong) is **under-search** or **structurally drawish**. This is stacked on PR #94; it does not use random bots or scripted tactics.

## Method

1. **Logs**: Release `apa-kan-bench`, `--no-default-features`, calibrated light budget `nodes:132000,iters:2600`, minimax vs minimax, `--objective capture-score`, `eval_capture_score=0` (enhanced / exactplay-equivalent defaults).
2. **Smoke source**: `smoke_base_mm.jsonl.gz` — 4 games, all **turn_limit** (2 draws at 4–4, 2 one-point turn-limit wins). Replays per `bench_log::replay_to_action`.
3. **Positions**: Up to 4 plies per game at late **tied / ±1** capture scores (focus t≥80 on stalled games). **n=16** positions.
4. **Search**: Same bot config on replayed states, repetition hash from JSONL. Per position:
   - Minimax at **1× / 16× / 64×** node budgets (132k / 2.11M / 8.45M nodes).
   - Optional **fan-out** at max mult (64×): **4** independent 64× searches with root move shuffle + different seeds (`PROBE_FANOUT=4`, `PROBE_FANOUT_OFF=1` to disable).
   - Parallel `--probe-jobs 8`.
5. **Signals**:
   - `ch16` / `ch64`: best move differs from 1×.
   - `term1`: completed root score has `|sc| ≥ WIN/2` (proven terminal in tree).
   - `ic`: count of one-ply captures in legal search moves.
   - `co` / `cs`: claimable overwhelmed opponent / own tiles (local-claim setup).

Tooling: `apa-kan-bench --deep-probe`, `run_deep_probe.sh`, raw TSVs `deep_probe_*_nofanout.txt` / `deep_probe_*_fanout.txt`.

## Base-mode smoke game context (seed 901)

| n | P1 W-D-L | mean turns | mean score | end |
|---|----------|------------|------------|-----|
| 4 | 0-2-2 | 200 | 2.75–4.00 (margin 1.25) | 0 target / 4 turn_limit |

Bench play: **83% outer** destination rings, 6.75 opp captures/game but still 4–4 caps in two draws — bots recycle threats without finishing to 6.

## Deep probe results (smoke_base, n=16)

| Metric | Value |
|--------|------:|
| Move change **16×** vs 1× | **37.5%** (6/16) |
| Move change **64×** vs 1× | **50.0%** (8/16) |
| **64×** completed ID depth (median / max) | **8 / 11** plies @ 8.45M nodes |
| **16×** completed ID depth (median / max) | **7 / 10** plies @ 2.11M nodes |
| **1×** completed ID depth | **4–6** plies @ 132k nodes |
| Root scores at 64× | **±180–420** (heuristic), **never** `term1` |
| `deep_only_terminal_win_flags` | **0** |
| Positions with immediate capture in move list (`ic>0`) | **10/16** |
| Those where 16× or 64× changed the pick vs 1× | **6/10** |
| Overwhelmed claimable at probe (`co>0`) | **0/16** |
| Dest ring (1× light pick) | inner 6, outer 10 |
| Dest ring (64× pick) | inner 7, outer 9 |

### Forced captures / score wins?

- **No.** At 8.45M nodes and depth 8–11, **no** position gets a minimax root score in the **±50k terminal band**. There are **no** hidden score-target wins or forced capture ladders that light search misses at these late tied scores.
- **Immediate captures** exist tactically (`ic` up to 19 on one ply), but deeper search **re-routes** rather than revealing a winning sequence; examples: `g1 t80` light `move22->18` vs 64× `move1->16`; `g4 t82` light `move12->13` vs 64× `move10->19` (same heuristic root score sign ~±180 at limit).
- **Local-claim knockouts** (`co>0`) are **absent** on all 16 samples: endgame tension is **movement / overwhelm setup**, not sitting on a claimable tile. Mutual safety here is **search-visible**: both sides’ scores stay in the **hundreds** in eval, not terminal.

### Board spread vs depth

- Deeper search **does not** open the board: destination rings stay **outer-heavy** (62% outer at 1× → 56% at 64×). The small inner bump (6→7) is reordering among **inner ping-pong / wrap moves**, not a shift to center fights (`center` ring count 0 on all picks).

### Example rows (abbrev.)

| game | turn | scores | 1× → 64× | d₁ | d₆₄ | nodes₆₄ |
|------|------|--------|----------|---:|----:|--------:|
| g1 | 80 | 3–4 | `move22->18` → `move1->16` | 6 | 10 | 8.45M |
| g2 | 66 | 1–3 | `move1->6` → `move1->21` | 4 | 7 | 8.45M |
| g4 | 83 | 2–3 | `move17->14` → `move17->24` | 4 | 8 | 8.45M |
| g3 | 160 | 3–4 | `move19->15` → `move12->9` | 4 | 7 | 8.45M |

Full TSV: `deep_probe_base_nofanout.txt`.

## Randomized fan-out (64×) vs single-seed 64× — **base**

| Arm | `PROBE_FANOUT` | `ch64` | `ch16` | `term1` | `fan_uniq_mean` (max) |
|-----|----------------|-------:|-------:|--------:|----------------------:|
| Single 64× seed | **0** (`PROBE_FANOUT_OFF=1`) | **50%** | **37.5%** | **0** | **1.0** |
| **4× shuffled 64×** | **4** | **50%** | **37.5%** | **0** | **2.75** (**4**) |

**Interpretation:** fan-out **does** surface **2–4** tied 64× candidates per stalled line (`fan64_uniq` distribution: 1×3, 2×3, 3×5, 4×5 over 16 positions), but **none** of the seeds pick a different **class** of outcome: `ch64` vs 1× stays **8/16**, still **zero** `term1`, same imm-capture / `co=0` profile. Randomized fan-out confirms **move instability** (heuristic plateaus) without revealing a **hidden score win** light search skipped.

TSVs: `deep_probe_base_nofanout.txt`, `deep_probe_base_fanout.txt` (`compare_probe.py`).

## Qualitative play read (smoke JSONL)

Scripts: `qualitative_read.py`, `survey.py` on committed logs.

### Base (`smoke_base_mm`, 4×200 turn_limit)

- **Center sitting:** almost no center **ADDs** (0); **34** MOVEs touch center across 800 actions vs **19** center-dest moves in survey — chips **pass through** center rarely; **2/4** finals leave center occupied.
- **Counterplay:** high **opp capture** rate (6.8/game) but scores still hug **4–4**; **0** CLAIM actions — tension is **movement-to-overwhelm**, not claim execution at sampled lines. **No** immediate reverse-move pairs (`A→B` then `B→A` back-to-back).
- **Piece types:** play is **low-weight dot tiles** (`norm2 w2`, `braid2`, `zag1` dominate ADD/MOVE). Heavy w3 braids show on long MOVE grinds, not opening fights.

### Majority spawn (`smoke_majority_mm`, local 4-game smoke)

- **Center / inner:** destination mix shifts **center 7%** (55 center-dest), **inner 21%** (vs base **3% / 25%**). **98** MOVEs touch center — more **mid-board traffic** with spawn tokens spent (`majority_spawn` awards **260**).
- **Finishing:** **1/4** hits **score target** at **t=34** (6–5 comeback); others still hit turn_limit. Mean turns **158** (pulls down by fast game).
- **Counterplay:** captures/game **8.2** opp; **2/3** decided games are **comebacks** on capture-score.

### Capture cascade (`smoke_cascade_mm`, local 4-game smoke)

- **Ratchets:** **10** capture-ratchet rewards; **4 SWAP**s appear (sparse-board capture follow-ups).
- **Passivity:** still **3/4** games hit **turn_limit** or low capture volume (**3.2** opp caps/game) — cascade **helps finish one** game (**t=47** score win) but does not globally prevent stalls.

### Unlimited spawn (`smoke_unlimited_mm`, local 4-game smoke)

- **Finishing:** **3/4** score wins (t=21/31/126); one **5–5 turn_limit** draw at t=200 (fast collapses, not grind loops).
- **Center:** minimal traffic (**12** MOVEs touch center, **9%** center-dest); play stays **outer-heavy** even with bench refills.
- **Piece types:** **zag1** / **bead1** ADD diversity (w1–w3); MOVE grind still **norm2 w2** + **zag1 w3** + **braid2**.
- **Counterplay:** **10.0** opp captures/game (high churn); no CLAIMs; unlimited bench does **not** force center occupation in these minimax lines.

## Deep probe — **majority spawn** (n=12 late positions, fan-out off)

| Metric | Base stall | Majority |
|--------|----------:|---------:|
| `ch64` vs 1× | 50% | **66.7%** |
| `ch16` vs 1× | 37.5% | **50.0%** |
| `term1` | 0 | **0** |
| `co>0` | 0/16 | **0/12** |
| Root \|sc\| @ 64× | ~400 | **~700–1000** (higher material/spawn tension) |

Still **no** terminal band, but eval **magnitude** rises (spawn tokens spent, center-adjacent ADD/MOVE like `move20->0` in probe lines). Deeper search **disagrees with light play more often** yet does **not** fabricate a forced target-6 win on these late lines.

**Majority fan-out (4× @64×):** `fan_uniq_mean=2.58` (max **4**); `ch16/ch64` **identical** to single-seed (`compare_probe.py`); **`term1=0`**. Fan-out surfaces inner-ring ties (`move20->0`, `move11->6` vs `move11->21`) without a hidden terminal score line.

## Deep probe — capture cascade (n=12, fan-out 4 @64×)

| Metric | Base | Cascade |
|--------|-----:|--------:|
| `ch64` vs 1× | 50% | **75.0%** |
| `ch16` vs 1× | 37.5% | **66.7%** |
| `term1` | 0 | **0** |
| `co>0` | 0/16 | **0/12** |
| `fan_uniq_mean` | 2.75 | **2.33** (max 4) |

Ratchet / SWAP **finish** one smoke game early (t=47), but **late tied** lines probed here still show **no** terminal root scores at 64×—deeper search **reorders** outer shuffles (`move19->18` vs `move19->13`, `move24->1`) without a forced target-6 line. TSV: `deep_probe_cascade_fanout.txt`.

## Deep probe — unlimited spawn (n=8 late, fan-out 4 @64×)

Most unlimited games end by **score** quickly; only **one** 5–5 turn_limit supplies late tied samples (**n=8**). On that grind:

| Metric | Base | Unlimited (late 5–5) |
|--------|-----:|---------------------:|
| `ch64` vs 1× | 50% | **50.0%** |
| `ch16` vs 1× | 37.5% | **37.5%** |
| `term1` | 0 | **0** |
| `fan_uniq_mean` | 2.75 | **2.38** |
| Root \|sc\| @ 64× | ~400 | **~40–100** (near-draw eval) |

High one-ply capture counts (`ic` up to 33) but **`term1=0`**; fan-out disagrees on inner vs outer ties (`move11->6` vs `move11->12`). TSV: `deep_probe_unlimited_fanout.txt`.

## Variant fan-out sweep (summary)

| Log | n | `ch64` | `term1` | `fan_uniq_mean` |
|-----|--:|-------:|--------:|----------------:|
| base | 16 | 50% | 0 | 2.75 |
| majority | 12 | 67% | 0 | 2.58 |
| cascade | 12 | 75% | 0 | 2.33 |
| unlimited (late) | 8 | 50% | 0 | 2.38 |

Across rules levers, **randomized 64× fan-out never introduces a terminal score win** that single-seed 64× missed; it only widens **tied-move** ambiguity.

## Verdict

**Base capture-score passivity is largely structural, not a shallow-search artifact.** Late tied games show **no** terminal winning lines even at **64×** calibrated nodes and **depth 11**; search only **swaps among safe outer/inner shuffles** (~50% of positions change the exact move at 64×). Tactical capture **choices** exist one-ply, but they do not compose into score-target wins in the search tree — consistent with **4–4 turn-limit stalls** under `eval_capture_score=0`. Deeper search is still useful for **move stability** (fan-out + 16× needed for agreement), not for uncovering a forced breakthrough.

**Not under-search** in the sense of “one hidden winning tactic”; **yes under-incentivized** — without spawn/ratchet levers (cf. majority_spawn / cascade on #94), rational search **accepts heuristic draws** and ping-pong. Majority and cascade **raise** `ch64` (more depth disagreement) and **finish** some smoke games faster, but probed **late tied** lines remain **`term1=0`** with **`co=0`**—same structural passivity class as base.

## Reproduce

```bash
cargo build --release --no-default-features -p apa-kan --bin apa-kan-bench

# games (if logs missing)
apa-kan-bench --no-depth-probe --games 4 --seed 901 --bots minimax --pairings minimax:minimax \
  --objective capture-score --out apa-kan/game/benchmark_results/capture_score/smoke_base_mm.txt

# probe
PROBE_FANOUT=0 PROBE_JOBS=8 ./apa-kan/game/benchmark_results/capture_score/run_deep_probe.sh \
  apa-kan/game/benchmark_results/capture_score/smoke_base_mm.jsonl.gz
```