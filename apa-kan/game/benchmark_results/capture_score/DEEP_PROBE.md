# Capture-score deep search probe

Independent check of whether **base** capture-score stalling (4–4 at turn 200, inner-ring move ping-pong) is **under-search** or **structurally drawish**. This is stacked on PR #94; it does not use random bots or scripted tactics.

## Method

1. **Logs**: Release `apa-kan-bench`, `--no-default-features`, calibrated light budget `nodes:132000,iters:2600`, minimax vs minimax, `--objective capture-score`, `eval_capture_score=0` (enhanced / exactplay-equivalent defaults).
2. **Smoke source**: `smoke_base_mm.jsonl.gz` — 4 games, all **turn_limit** (2 draws at 4–4, 2 one-point turn-limit wins). Replays per `bench_log::replay_to_action`.
3. **Positions**: Up to 4 plies per game at late **tied / ±1** capture scores (focus t≥80 on stalled games). **n=16** positions.
4. **Search**: Same bot config on replayed states, repetition hash from JSONL. Per position:
   - Minimax at **1× / 16× / 64×** node budgets (132k / 2.11M / 8.45M nodes).
   - Optional **fan-out** at max mult: re-search with root move shuffle + independent seeds (`--probe-fanout`, off for this smoke run due to runtime).
   - Parallel `--probe-jobs 8`.
5. **Signals**:
   - `ch16` / `ch64`: best move differs from 1×.
   - `term1`: completed root score has `|sc| ≥ WIN/2` (proven terminal in tree).
   - `ic`: count of one-ply captures in legal search moves.
   - `co` / `cs`: claimable overwhelmed opponent / own tiles (local-claim setup).

Tooling: `apa-kan-bench --deep-probe`, `run_deep_probe.sh`, raw TSV `deep_probe_results.txt`.

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

Full TSV: `deep_probe_results.txt`.

## Majority-spawn comparison

_Queued_: `smoke_majority_mm.jsonl.gz` (games finish ~73 turns with score-target ends). Re-run:

```bash
./run_deep_probe.sh apa-kan/game/benchmark_results/capture_score/smoke_majority_mm.jsonl.gz
```

Expect fewer late tied stalls and more `co>0` / inner traffic when spawn incentive breaks the draw pocket.

## Verdict

**Base capture-score passivity is largely structural, not a shallow-search artifact.** Late tied games show **no** terminal winning lines even at **64×** calibrated nodes and **depth 11**; search only **swaps among safe outer/inner shuffles** (~50% of positions change the exact move at 64×). Tactical capture **choices** exist one-ply, but they do not compose into score-target wins in the search tree — consistent with **4–4 turn-limit stalls** under `eval_capture_score=0`. Deeper search is still useful for **move stability** (fan-out + 16× needed for agreement), not for uncovering a forced breakthrough.

**Not under-search** in the sense of “one hidden winning tactic”; **yes under-incentivized** — without spawn/ratchet levers (cf. majority_spawn on #94), rational search **accepts heuristic draws** and ping-pong.

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