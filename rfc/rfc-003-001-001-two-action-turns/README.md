# RFC-003-001-001: Two Actions Per Turn

## Problem

Under the benchmarked one-action rules, a lone tile can be wiped immediately by an adjacent mismatched ADD (corners count as a mismatch, so a neighbor on a corner-adjacent edge often removes both tiles). Building a matching group is too hard because the opponent replies before you can back the new tile up.

## Proposal

A **two-action turn** variant: the same player may take up to two ADD/MOVE actions before the opponent replies. Removals still run after **each** action, so the second action can save a tile that would otherwise die.

Existing one-action rulesets (`scored()`, `scored_7x7()`, `old()`, …) are unchanged: `actions_per_turn = 1`.

## Constructors

| Constructor | Base | `actions_per_turn` | `opening_handicap` |
|---|---|---|---|
| `RuleSet::scored_double()` | scored 5×5 | 2 | true |
| `RuleSet::scored_7x7_double()` | scored 7×7 | 2 | true |
| `RuleSet::old_double()` | old 5×5 | 2 | true |

## Design choices

1. **Removals after each action.** Same `check_and_remove_all_mismatched` as today. ADD-bank earning from MOVE captures is still evaluated per action.

2. **Opening handicap (Connect6-style), on by default.** Player One's first turn is only one action so they cannot immediately plant a backed-up pair. `--no-opening-handicap` turns this off. Irrelevant when `actions_per_turn == 1`.

3. **Pass ends the whole turn**, even if an action remains. In two-action scored games Pass is always legal so a player can stop after one action; one-action scored games still only generate Pass when there is no legal ADD/MOVE. A *whole-turn pass* is Pass as the first action of the turn (`consecutive_passes` increments). Pass after ADD/MOVE only forfeits remaining actions and does not count toward double-pass. Game-ending double pass means both players passed a whole turn in a row (`consecutive_passes >= 2`).

4. **If a player has no legal ADD/MOVE left mid-turn** after already acting, the turn ends without counting as a pass (they already made a real action).

5. **Turn limit counts full turns.** `turn_count` increments when a player's turn ends, not after every action. One-action games still increment once per action, so the 100-turn limit is unchanged for existing rulesets.

6. **Bots.** Each action is one search ply. Minimax maximizes while `current_player == original_player` and minimizes otherwise, so a two-action turn is two consecutive maximizing (or minimizing) plies. MCTS nodes and rollouts call `apply_action`, which only switches the player after the last action of the turn.

7. **CLI.** `--actions <1|2>` selects the constructor. `--no-opening-handicap` only affects two-action games.

## Play

```bash
cargo run -p apa-kan -- --bot ologbon --actions 2 --rules scored --size 5 --think-ms 500
```

Opt out of the opening handicap with `--no-opening-handicap`.

## Quick benchmark

Copied from `apa-kan/game/benchmark_results/benchmark_double.txt` (think 50ms/action, 10 games/seat, opening handicap on, Total time 120.99s). Seat format is P1 wins-draws-P2 wins across both seats.

```
5x5 Ologbon vs Ologbon  games 20  P1 0-19-1 P2  MOVE 29.9%  avg turns 16.8  ends: double_pass 18  turn_limit 1  elimination 1  other_win 0  unfinished 0
5x5 Olusare vs Ologbon  games 20  P1 7-2-11 P2  MOVE 48.4%  avg turns 23.1  ends: double_pass 3  turn_limit 1  elimination 16  other_win 0  unfinished 0
5x5 Olusare vs Olusare  games 20  P1 14-0-6 P2  MOVE 70.8%  avg turns 40.4  ends: double_pass 2  turn_limit 4  elimination 14  other_win 0  unfinished 0
7x7 Ologbon vs Ologbon  games 20  P1 2-0-18 P2  MOVE 66.8%  avg turns 31.6  ends: double_pass 7  turn_limit 4  elimination 9  other_win 0  unfinished 0
7x7 Olusare vs Ologbon  games 20  P1 10-2-8 P2  MOVE 48.7%  avg turns 23.4  ends: double_pass 6  turn_limit 1  elimination 13  other_win 0  unfinished 0
7x7 Olusare vs Olusare  games 20  P1 2-1-17 P2  MOVE 86.4%  avg turns 80.2  ends: double_pass 0  turn_limit 13  elimination 7  other_win 0  unfinished 0
```
