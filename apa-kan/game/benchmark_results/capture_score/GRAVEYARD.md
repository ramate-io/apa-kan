# Graveyard and capturer-owns variants (PR #103)

Everything is 150 games, minimax vs minimax, seed 300, release, `--no-default-features`, SUM, on top of `--objective capture-score --unlimited-spawn on`. Per-game JSONL in this folder replays every game; see `committed_capture_score_graveyard_replays`.

- **combo:** `--opening-double off --graveyard on --capture-to-capturer on --score-target 5`.
- **Ablations:** one toggle each on unlimited spawn.
  - **graveyard:** `--graveyard on`.
  - **capturer:** `--capture-to-capturer on`. This keeps the 2-2-1-1 opening and target 6.
  - **t5:** `--score-target 5`.

Control reference (live control-cascade, same harness): P1 75-3-72 (0.500), median 40.5 turns.

| unlimited | 150 | 72-25-53 | 53-25-72 | 0.480 | 95.0 / 65.5 | 9.93 / 25.82 | 5.03 / 4.90 (abs 1.60) | score 113, draw turn_limit 25, turn_limit 8, locked_out 4 | 17 rep≥3 (avg max 5.44) | 74/125 (59.2%) | 2.7/19.0/57.1/21.3 | 6.5/8.6/73.1/11.8 | 0.0 / 44 / 41/122 | 0.0 |
| combo | 150 | 65-26-59 | 59-26-65 | 0.433 | 107.4 / 84.0 | 8.20 / 17.25 | 4.11 / 4.09 (abs 1.24) | score 106, draw turn_limit 26, turn_limit 11, locked_out 7 | 18 rep≥3 (avg max 3.02) | 83/124 (66.9%) | 3.0/20.3/54.3/22.5 | 4.6/13.8/68.9/12.6 | 0 / 37 / 27/119 | 22.3 |
| graveyard | 150 | 52-33-65 | 65-33-52 | 0.347 | 109.8 / 86.5 | 10.03 / 14.47 | 4.91 / 5.12 (abs 1.37) | score 106, draw turn_limit 33, locked_out 8, turn_limit 3 | 9 rep≥3 (avg max 1.48) | 80/117 (68.4%) | 3.2/22.1/52.1/22.6 | 5.9/11.3/65.7/17.1 | 0 / 38 / 34/111 | 21.4 |
| capturer | 150 | 34-20-96 | 96-20-34 | 0.227 | 129.7 / 114.0 | 8.04 / 39.11 | 3.38 / 4.66 (abs 2.21) | score 85, turn_limit 35, draw turn_limit 20, locked_out 10 | 21 rep≥3 (avg max 7.54) | 43/130 (33.1%) | 2.5/19.6/55.2/22.7 | 3.6/15.9/59.4/21.1 | 53 / 96 / 62/127 | 0.0 |
| t5 | 150 | 75-21-54 | 54-21-75 | 0.500 | 82.3 / 54.5 | 8.15 / 22.91 | 4.21 / 3.95 (abs 1.39) | score 117, draw turn_limit 21, locked_out 7, turn_limit 5 | 16 rep≥3 (avg max 5.08) | 79/129 (61.2%) | 3.0/17.6/59.4/20.1 | 6.7/9.9/72.4/11.0 | 0 / 36 / 37/125 | 0.0 |

Columns:
- **dest / capture C/I/S/O:** center, inner ring, spawn ring, outer ring.
- **lead tail:** turns from the moment the winner took the lead for good to the end of the game. It shows the median, the count of games where the tail is 10 turns or more, and how many losers still captured after that point.

## Snowball and pace analysis

**Center lock (Liam's complaint about control).**

| Cell | winner held the center to the end | median turns held |
|---|---|---|
| control | 78/147 | 11.5 (45 games ≥10) |
| unlimited | 25/125 | 19 |
| combo | 25/124 | 43 |
| graveyard | 20/117 | 24 |
| t5 | 30/129 | 13 |
| capturer | 48/130 | 41.5 |

Every capture-score cell breaks the "take the center, then wait" pattern. The winner sits the center at the end in a quarter or less of games, against more than half under control.

**Match-point standoff.** The main pace problem: games reach a tied (target−1)-all score and then stall.

| Cell | games reaching tied match point | median turns afterwards | of those, hit the cap |
|---|---|---|---|
| unlimited | 93 | 45 | 25 |
| combo | 102 | 40 | 26 |
| graveyard | 100 | 40 | 32 |
| t5 | 98 | 25.5 | 21 |
| capturer | 36 | 32.5 | 12 |

In those games, captures trade 1-for-1 against self-captures while both sides refuse the last exchange. Seed 10300 under combo is the clearest example: 4-4 at turn 45, then 72 turns of standoff before it was decided.

**Graveyard.**
- It breaks loops: 9 games with rep≥3 (avg max 1.48), against 17 (5.44) for unlimited.
- It raises comebacks to 68% and plays about 21 RECOVER actions per game. Kind entropy rises from 1.02 to 1.50, because RECOVER is a real timing decision.
- It costs about 20 turns (median 86.5 against 65.5) and tilts to P2 (0.347).

**Capturer-owns alone is the worst cell, and it snowballs.**
- P1 wins 0.227. The lead tail median is 53 turns, 96 games have a tail of 10 or more, and comebacks drop to 33%.
- With the 2-2-1-1 opening still on, P2 makes the first opponent capture in 131/150 games, usually on turn 4, and wins 90 of those 131. Every capture also adds a tile to the capturer's hand, so the first capture compounds.
- Self-captures jump to 39 per game. Tiles you capture become yours, so they are cheap to recycle through your own bench.
- Inside the combo, the graveyard tax and the dropped opening neutralize this.

**Target 5.**
- The fastest and fairest cell: P1 0.500, median 54.5 turns, 117 target wins, comebacks 61%.
- The match-point wait halves to a median of 25.5.

## Variety

- Every cell's dominant capture ring is the spawn ring, in 128–144 of 150 games. Plans are still mostly spawn starving, with some inner-ring raids. The graveyard adds a recycle-timing layer.
- Ring use per tile shifts to more 1-ring tiles under graveyard and combo (1139–1673 single-ring placements, against 589–757).

## Take

None of these cells beats control on pace. All of them beat it on the snowball Liam described: the center lock is gone, comebacks run 59–68% where the seats are balanced, and losers keep capturing after the decisive swing in about a quarter to a third of games.

The best base is **unlimited spawn + target 5, without capturer-owns**, optionally with the graveyard if loops matter more than pace. Capturer-owns needs the opening removed or a tax, otherwise it is a first-capture snowball. The match-point standoff is the next thing to fix. Repetition-ends (#105) and no-immediate-return (#106) target it.
