# Search H2H logs

Release `apa-kan-bench`, 12 games per pairing per seat, 1000 ms, seed 1, `--no-depth-probe`, 108 games, zag mismatch SUM. Mean avg turns is the unweighted mean of the nine pairing `avg_turns` columns.

## Center double-edge scoring + new low-value (current)

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
