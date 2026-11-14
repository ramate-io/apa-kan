# Search H2H logs

Release `apa-kan-bench`, 12 games per pairing per seat, seed 1, `--no-depth-probe`, 108 games, zag mismatch SUM. Mean avg turns is the unweighted mean of the nine pairing `avg_turns` columns.

## Work budget (reproducible default)

`--budget nodes:132000,iters:2600` is the default. It is the rounded mean of a **single-threaded** 1000 ms/move calibrate on this VM (4 physical cores), so the same seed replays regardless of load. `--jobs` defaults to `available_parallelism` (4 here); each search stays single-threaded; JSONL is written in pairing-then-game order.

Calibrate log: [`work_budget_calibration.txt`](work_budget_calibration.txt). Command: `apa-kan-bench --calibrate --think-ms 1000 --seed 1` (3 games × 16 moves, minimax vs MCTS). Minimax nodes/move: n=24, mean=131781, min=46720, max=248256, mean completed ID depth=3.58. MCTS iters/move: n=24, mean=2635, min=1327, max=3961. `--think-ms` remains for wall-clock search (not reproducible).

Release `apa-kan-bench`, 12/pairing/seat, seed 1, `--no-depth-probe`, 108 games, zags on, `--jobs 4`. Wall-clock: allowed **1796.2 s** (~30 min), disallowed **2032.8 s** (~34 min), both **3829 s** (~64 min). Time-based pair on this VM was 1048.4 + 1050.5 s (~35 min).

| Dir | P1 W-D-L | Ends (control/lockout/TL/LV/elim) | Mean avg turns | Opp / self /game | Center opp / self | Log |
|---|---|---|---|---|---|---|
| [`work_budget_doubles_allowed/`](work_budget_doubles_allowed/) | **45-17-46** | 38 / 7 / 14 / 49 (wins 46, draws 3) / 0 | **60.29** | 5.15 (556) / 5.06 (546) | 0.59 (64) / 0.16 (17) | work budget, doubles allowed |
| [`work_budget_doubles_disallowed/`](work_budget_doubles_disallowed/) | **50-15-43** | 37 / 10 / 11 / 50 (wins 46, draws 4) / 0 | **57.03** | 5.12 (553) / 5.13 (554) | 0.58 (63) / 0.19 (21) | `--no-same-piece-twice when-deck-empty` |
| [`center_double_edge_doubles_allowed/`](center_double_edge_doubles_allowed/) | **53-8-47** | 38 / 9 / 5 / 56 (wins 53, draws 3) / 0 | **47.47** | 5.42 (585) / 5.04 (544) | 0.58 (63) / 0.11 (12) | time-based 1000 ms |
| [`center_double_edge_doubles_disallowed/`](center_double_edge_doubles_disallowed/) | **55-12-41** | 34 / 9 / 8 / 57 (wins 53, draws 4) / 0 | **51.45** | 5.38 (581) / 5.30 (572) | 0.58 (63) / 0.14 (15) | time-based 1000 ms |

Work-budget games run longer (more turn-limits and draws) than the time-based pair: that 1000 ms was 9 pairing threads on 4 cores, so each search got less than a full second of CPU. The work budget is a full single-thread 1000 ms. Captures and center pops stay in the same band. P1 W-D-L shifts toward more draws (8→17 allowed, 12→15 disallowed).

## Center double-edge scoring + new low-value (time-based, historical)

Inner↔center is two generating-circle petals, so scoring walks `Board::incident_others`. Low-value fires when both players have `board + usable budget < 5` (usable need not be 0). These runs were recorded **after** both of those changes.

| Dir | Rules | P1 W-D-L | Mean avg turns | Notes |
|---|---|---|---|---|
| [`center_double_edge_doubles_allowed/`](center_double_edge_doubles_allowed/) | independent channels, `--braid-fail-mismatch one`, `--no-same-piece-twice off` | 53-8-47 | 47.47 | accepted default |
| [`center_double_edge_doubles_disallowed/`](center_double_edge_doubles_disallowed/) | same, `--no-same-piece-twice when-deck-empty` | 55-12-41 | 51.45 | comparison |

## Unique-neighbor logs (historical)

These files predate the inner↔center double-edge scoring fix **and** the `board + usable < 5` low-value trigger. Unique `neighbors()` collapsed the two petals to one adjacency, so a failed-braid `one` only cost +1 on that pair, and low-value required both usable budgets to be 0. They are **kept for comparison** and must not be overwritten.

| File | Same as | P1 W-D-L | Mean avg turns |
|---|---|---|---|
| [`doubles_allowed.txt`](doubles_allowed.txt) | default package **before** the double-edge fix and the new low-value trigger | 46-18-44 | 68.51 |
| [`independent_channels.txt`](independent_channels.txt) | `--no-same-piece-twice when-deck-empty` before those changes | 42-19-47 | 75.11 |
| [`doubles_disallowed.txt`](doubles_disallowed.txt) | byte-for-byte copy of `independent_channels` | 42-19-47 | 75.11 |
| [`braid_fail_weight.txt`](braid_fail_weight.txt) / [`braid_fail_sum.txt`](braid_fail_sum.txt) | research `--braid-fail-mismatch` variants ([#39](https://github.com/ramate-io/apa-kan/issues/39)), also unique-neighbor | — | — |

JSONL from those unique-neighbor runs still decodes. Full replay is skipped (`score_incident_arcs` absent) because captures would not match per-arc scoring.
