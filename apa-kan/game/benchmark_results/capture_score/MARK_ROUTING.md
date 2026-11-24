# Mark-capture routing (PR #105)

All cells: 150 games, minimax vs minimax, seed 300, release, `--no-default-features`, SUM.

Routing cells use `--objective capture-score --unlimited-spawn on --opening-double off --score-target 5 --graveyard on --mark-capture-routing <mode>`. The acting tile's mark decides where each captured tile goes, and CLAIM counts as a triangle:

| mode | triangle self-capture | triangle opponent capture | dot (either) |
|---|---|---|---|
| A | your hand | their graveyard | your graveyard |
| A-discard | your hand | out of the game | your graveyard |
| B | their graveyard (they own it) | your hand (you own it) | your graveyard |

References, both from #103:
- **t5:** unlimited spawn + target 5, with the 2-2-1-1 opening and no graveyard.
- **combo:** no 2-2-1-1, graveyard, capturer-owns, target 5.

Control: P1 0.500, median 40.5 turns.

| Cell | n | P1 W-D-L | P2 W-D-L | P1 WR | mean / med turns | opp / self caps per game | mean score P1 / P2 (margin) | end reasons | repetition | comebacks | dest C/I/S/O % | capture C/I/S/O % | lead tail med / ≥10 / loser counterplay | RECOVER/game |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| t5 (no routing) | 150 | 75-21-54 | 54-21-75 | 0.500 | 82.3 / 54.5 | 8.15 / 22.91 | 4.21 / 3.95 (abs 1.39) | score 117, draw turn_limit 21, locked_out 7, turn_limit 5 | 16 rep≥3 (avg max 5.08) | 79/129 (61.2%) | 3.0/17.6/59.4/20.1 | 6.7/9.9/72.4/11.0 | 0 / 36 / 37/125 | 0.0 |
| combo (#103) | 150 | 65-26-59 | 59-26-65 | 0.433 | 107.4 / 84.0 | 8.20 / 17.25 | 4.11 / 4.09 (abs 1.24) | score 106, draw turn_limit 26, turn_limit 11, locked_out 7 | 18 rep≥3 (avg max 3.02) | 83/124 (66.9%) | 3.0/20.3/54.3/22.5 | 4.6/13.8/68.9/12.6 | 0 / 37 / 27/119 | 22.3 |
| mark A | 150 | 60-31-59 | 59-31-60 | 0.400 | 102.5 / 74.0 | 8.05 / 26.18 | 4.11 / 3.95 (abs 1.48) | score 107, draw turn_limit 31, turn_limit 7, locked_out 5 | 17 rep≥3 (avg max 4.82) | 68/119 (57.1%) | 2.3/19.2/58.8/19.8 | 3.6/9.6/74.8/12.0 | 0.5 / 45 / 32/116 | 10.2 |
| mark A-discard | 150 | 70-26-54 | 54-26-70 | 0.467 | 102.0 / 78.0 | 7.51 / 26.17 | 3.90 / 3.61 (abs 1.81) | score 109, draw turn_limit 26, turn_limit 12, locked_out 3 | 17 rep≥3 (avg max 4.57) | 57/124 (46.0%) | 2.5/19.5/56.7/21.3 | 4.3/11.1/71.3/13.2 | 17.0 / 71 / 43/122 | 6.6 |
| mark B | 150 | 65-23-62 | 62-23-65 | 0.433 | 98.1 / 74.5 | 8.08 / 13.93 | 4.03 / 4.05 (abs 1.47) | score 116, draw turn_limit 23, turn_limit 7, locked_out 4 | 8 rep≥3 (avg max 1.85) | 88/127 (69.3%) | 3.1/21.5/52.0/23.4 | 2.6/12.7/68.8/15.8 | 0 / 48 / 34/125 | 14.5 |
| B + repetition-ends | 150 | 64-24-62 | 62-24-64 | 0.427 | 94.9 / 74.0 | 8.08 / 12.25 | 4.03 / 4.05 (abs 1.47) | score 116, draw turn_limit 22, locked_out 4, repetition 3, turn_limit 3, draw repetition 2 | 5 rep≥3 (avg max 1.20) | 88/126 (69.8%) | 3.1/21.6/51.1/24.2 | 2.6/12.7/68.6/16.0 | 0.0 / 47 / 33/124 | 12.9 |

## Do triangles and dots diverge into distinct plans?

Yes under A and A-discard, more mildly under B. Per-game counts (`markqual.py`):

| Cell | self-captures tri / dot | ADD tri / dot | MOVE tri / dot | triangle share of a player's opponent captures: mean (winners / losers) | RECOVER |
|---|---|---|---|---|---|
| t5 (no routing) | 11.0 / 12.0 | 19.4 / 19.9 | 23.1 / 20.6 | 0.53 (0.50 / 0.55) | 0 |
| combo | 9.6 / 7.7 | 18.4 / 15.8 | 27.1 / 24.3 | 0.52 (0.50 / 0.54) | 22.3 |
| A | **21.9 / 4.3** | **30.9 / 11.6** | 28.3 / 22.1 | 0.62 (0.59 / 0.64) | 10.2 |
| A-discard | **21.6 / 4.6** | **29.6 / 12.4** | 30.7 / 23.4 | 0.52 (0.53 / 0.49) | 6.6 |
| B | 6.3 / 7.7 | 16.2 / 14.4 | 26.6 / 26.8 | 0.58 (0.55 / 0.64) | 14.5 |

- **A and A-discard turn triangles into a recycling engine.** A triangle self-capture goes straight back to hand. Triangle self-captures double, and triangle ADDs outnumber dot ADDs 2.6 to 1. Dots become the board pieces: they move about twice as often as they are added, and their self-captures nearly vanish because the tile sinks into your own graveyard. That is a real split: triangles are tempo and re-spawn, dots are positional. It is also mostly self-capture churn, 26 per game.
- **B punishes triangle self-captures.** They feed the opponent's graveyard, so triangle self-captures drop to 6.3 per game and total self-captures halve (13.9 against 26). A triangle's opponent capture lands in the capturer's hand, and 81% of those tiles are re-ADDed within three of the capturer's turns, so the steal is a tempo play. Winners lean slightly more on dots than losers (0.55 against 0.64 triangle share). Dot captures, which you own in your graveyard, are the slower and more lasting gain.
- The distribution of each player's triangle share is wide in every routing cell (the histogram spans 0–1, with no single peak), so the bots do mix plans from game to game. It is not bimodal, though: there is no clean "triangle player" against "dot player".
- Captures still happen mostly on the spawn ring (dominant ring S in 139–145 of 150 games).

## Snowballs, comebacks, standoff, pace

| Cell | comebacks | lead tail: median / games ≥10 / loser still captures after the lock | tied match point reached / median wait / hit the cap | winner held the center to the end | games with rep≥3 |
|---|---|---|---|---|---|
| t5 | 61.2% | 0 / 36 / 37 | 98 / 25.5 / 21 | 30/129 | 16 |
| combo | 66.9% | 0 / 37 / 27 | 102 / 40 / 26 | 25/124 | 18 |
| A | 57.1% | 0.5 / 45 / 32 | 91 / 41 / 30 | 16/119 | 17 |
| A-discard | **46.0%** | **17 / 71** / 43 | 66 / 50.5 / 22 | 26/124 | 17 |
| B | **69.3%** | 0 / 48 / 34 | 91 / 35 / 22 | 34/127 | **8** |

- **A-discard is the snowball mode.** Removing captured tiles makes a material lead permanent. The first +2 holder won 97/133, and the lead tail median is 17 turns. Fewer games reach the tied match point, but the ones that do wait longest.
- **B has the most comebacks of any capture-score cell so far (69%) and the fewest loops (8 games with rep≥3).** It is close to fair: 65-23-62.
- **Pace.** No routing mode is faster than plain t5 (median 74–78 against 54.5). All are about 10 turns faster than the combo (84). The graveyard costs tempo whichever way tiles reach it.
- **The match-point standoff remains.** Around 60% of games reach (target−1)-all and then wait a median of 35–50 turns. Routing doesn't fix it, so the repetition-ends and no-immediate-return cells are the pace test.

Examples (B):
- Seed 344: three lead changes, 4-5 in 75 turns.
- Seed 10324: three lead changes, 5-4 in 72 turns.

Example (A-discard): seed 300, P2 leads from the first capture and wins 1-5 with no lead change.

## B + repetition-ends (\`MARK_MODE=b unl_mark_rep_mm\`)

- **It doesn't break the 4-4 standoff.**
  - Only 5 of 150 games end on repetition: 3 won on points and 2 drawn at 4-4, with a median of 84 turns.
  - Tied match point is reached in 92 games, with a median wait of 36 (against 91 / 35 for B), and 21 of those still hit the cap.
  - The standoff isn't an exact position loop. The bots keep trading, recovering and shuffling, so the visible position (board, benches, graveyards, scores) rarely repeats three times.
  - B had already cut loops to 8 games with rep≥3; with the rule it is 5.
- **Pace barely moves:** mean 98.1 to 94.9 turns, median 74.5 to 74.0. Draws at the turn limit drop from 23 to 22.
- **Everything else matches B:** P1 0.427, comebacks 69.8%, the same mark split (self-captures 5.9 triangle / 6.4 dot per game; triangle share of opponent captures 0.58, with winners at 0.55 and losers at 0.64).
- **Verdict:** keep it as cheap insurance against exact loops, but it isn't a pace lever under B. The standoff needs a rule that changes incentives near match point, not one that detects repeats.

## Take

B is the best routing mode: the same pace as A, the most comebacks, the least self-capture spam and the fewest loops. It is the base for the repetition-ends and no-immediate-return combination cells (`MARK_MODE=b`). Neither combination changes pace or the standoff; see above and NO_RETURN.md (#106). A gives the clearest triangle/dot role split, but most of that split is self-capture churn. A-discard snowballs. None of them beats control on pace yet. All of them beat it on the center-lock snowball.
