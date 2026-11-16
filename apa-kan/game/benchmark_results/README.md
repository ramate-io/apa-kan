# Search H2H logs

Release `apa-kan-bench`, 12 games per pairing per seat, seed 1, `--no-depth-probe`, 108 games, zag mismatch SUM. Mean avg turns is the unweighted mean of the nine pairing `avg_turns` columns.

**Doubles allowed** (`--no-same-piece-twice off`) is the locked default and the **only configuration that is benchmarked**. `always` / `when-deck-empty` remain as non-default CLI variants; do not run them as the H2H package.

## Current (zag color exclusion, capture modifiers, work budget, doubles allowed)

`--budget nodes:132000,iters:2600` is the default. It is the rounded mean of a **single-threaded** 1000 ms/move calibrate on this VM (4 physical cores), so the same seed replays regardless of load. `--jobs` defaults to `available_parallelism` (4 here); each search stays single-threaded; JSONL is written in pairing-then-game order.

Calibrate log: [`work_budget_calibration.txt`](work_budget_calibration.txt). Command: `apa-kan-bench --calibrate --think-ms 1000 --seed 1` (3 games × 16 moves, minimax vs MCTS). Minimax nodes/move: n=24, mean=131781, min=46720, max=248256, mean completed ID depth=3.58. MCTS iters/move: n=24, mean=2635, min=1327, max=3961. `--think-ms` remains for wall-clock search (not reproducible).

Release `apa-kan-bench`, 12/pairing/seat, seed 1, `--no-depth-probe`, 108 games, `--jobs 4`, `--pick search`, 27-tile collections (`deck_noise=0`), `dot_percent=50`. Wall-clock **1708.8 s** (~28 min). Zag colors are excluded from normal / bead / braid matches.

| Dir | P1 W-D-L | Ends (control/lockout/TL/LV/elim) | Mean avg turns | Opp / self /game | Center opp / self |
|---|---|---|---|---|---|
| [`zag_color_exclusion/`](zag_color_exclusion/) | **52-5-51** | 30 / 27 / 3 / 47 (wins 45, draws 2) / 1 | **41.57** | 5.71 (617) / 5.96 (644) | 0.75 (81) / 0.13 (14) |

Capture dest: **584** to bench (5.41/game), **677** to tokens (6.27/game). Mean tokens/side **3.12**, mean live bench/side **6.26** (4598 snapshots). Draws: 2 low-value + 3 turn-limit. Picker (P1 opening dots of 11): 97/108 games had 5 dots (47-4-46); 6 dots 2-0-4; 4 dots 2-1-1; 3 dots 1-0-0.

Vs [`capture_modifiers/`](capture_modifiers/) (**49-7-52**, 41/18/2/47/0, mean **39.63**): fewer draws (7→5), more lockouts (18→27), fewer control wins (41→30), one elimination. Captures rise (opp 5.17→5.71, self 5.38→5.96) as same-color zag vs normal/bead stops washing out.

## Research: bead independent vs shared channel

[`bead_shared_channel/`](bead_shared_channel/) compares rule A (independent: normal+bead) with rule B (`--bead-scoring shared`: `max` instead of sum) on braid-heavy, bead-heavy, and mixed collections. Quote numbers from that directory. The 108-game default H2H above is unchanged (rule A).

## Historical: capture modifiers (before zag-color exclusion)

Same search budget and H2H shape, **before** zag colors were excluded from normal / bead / braid matches. Do not overwrite.

| Dir | P1 W-D-L | Ends (control/lockout/TL/LV/elim) | Mean avg turns | Opp / self /game | Center opp / self |
|---|---|---|---|---|---|
| [`capture_modifiers/`](capture_modifiers/) | **49-7-52** | 41 / 18 / 2 / 47 (wins 42, draws 5) / 0 | **39.63** | 5.17 (558) / 5.38 (581) | 0.58 (63) / 0.10 (11) |

Capture dest: **507** to bench (4.69/game), **632** to tokens (5.85/game). Mean tokens/side **3.09**, mean live bench/side **6.13** (4388 snapshots). Draws: 5 low-value + 2 turn-limit. Wall-clock **1630.6 s**.

Vs PR #40 [`work_budget_doubles_allowed/`](work_budget_doubles_allowed/) (**45-17-46**, 38/7/14/49/0, mean **60.29**): fewer draws (17→7), fewer turn-limits (14→2), more lockouts (7→18), games much shorter (60.29→39.63). Opp captures stay in band; self-captures rise (5.06→5.38) as dots recycle tiles onto the bench.

## Historical: work budget, doubles allowed (PR #40)

Same search budget and H2H shape, **before** 27-tile 7+4 draft, beads, and capture marks. Do not overwrite.

| Dir | P1 W-D-L | Ends (control/lockout/TL/LV/elim) | Mean avg turns | Opp / self /game | Center opp / self |
|---|---|---|---|---|---|
| [`work_budget_doubles_allowed/`](work_budget_doubles_allowed/) | **45-17-46** | 38 / 7 / 14 / 49 (wins 46, draws 3) / 0 | **60.29** | 5.15 (556) / 5.06 (546) | 0.59 (64) / 0.16 (17) |

Work-budget games on that package ran longer (more turn-limits and draws) than the earlier time-based doubles-allowed log: that 1000 ms was 9 pairing threads on 4 cores, so each search got less than a full second of CPU. The work budget is a full single-thread 1000 ms.

## Historical: time-based doubles allowed

Inner↔center is two generating-circle petals, so scoring walks `Board::incident_others`. Low-value fires when both players have `board + usable budget < 5` (usable need not be 0). This run used wall-clock `--think-ms 1000` (not reproducible).

| Dir | Rules | P1 W-D-L | Mean avg turns | Notes |
|---|---|---|---|---|
| [`center_double_edge_doubles_allowed/`](center_double_edge_doubles_allowed/) | independent channels, `--braid-fail-mismatch one`, `--no-same-piece-twice off` | 53-8-47 | 47.47 | previous accepted-default H2H |

## Historical: doubles disallowed

`--no-same-piece-twice when-deck-empty` is **not** benchmarked. These committed logs are kept for comparison and must not be overwritten.

| Dir / file | P1 W-D-L | Mean avg turns | Notes |
|---|---|---|---|
| [`work_budget_doubles_disallowed/`](work_budget_doubles_disallowed/) | 50-15-43 | 57.03 | work budget, `--jobs 4`; same method as the current allowed run |
| [`center_double_edge_doubles_disallowed/`](center_double_edge_doubles_disallowed/) | 55-12-41 | 51.45 | time-based 1000 ms, after double-edge scoring + new low-value |
| [`doubles_disallowed.txt`](doubles_disallowed.txt) | 42-19-47 | 75.11 | unique-neighbor / old low-value; byte-for-byte copy of `independent_channels` |

## Unique-neighbor logs (historical)

These files predate the inner↔center double-edge scoring fix **and** the `board + usable < 5` low-value trigger. Unique `neighbors()` collapsed the two petals to one adjacency, so a failed-braid `one` only cost +1 on that pair, and low-value required both usable budgets to be 0. They are **kept for comparison** and must not be overwritten.

| File | Same as | P1 W-D-L | Mean avg turns |
|---|---|---|---|
| [`doubles_allowed.txt`](doubles_allowed.txt) | default package **before** the double-edge fix and the new low-value trigger | 46-18-44 | 68.51 |
| [`independent_channels.txt`](independent_channels.txt) | `--no-same-piece-twice when-deck-empty` before those changes | 42-19-47 | 75.11 |
| [`braid_fail_weight.txt`](braid_fail_weight.txt) / [`braid_fail_sum.txt`](braid_fail_sum.txt) | research `--braid-fail-mismatch` variants ([#39](https://github.com/ramate-io/apa-kan/issues/39)), also unique-neighbor | — | — |

JSONL from those unique-neighbor runs still decodes. Full replay is skipped (`score_incident_arcs` absent) because captures would not match per-arc scoring.
