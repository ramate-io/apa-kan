# No-immediate-return (PR #106)

`--no-immediate-return on`: a tile may not MOVE, or initiate a SWAP, back onto the node it left on its own latest move, until its owner's next turn is over. There is no capture exception. A SWAP partner is not recorded, and a captured tile's record is dropped.

All cells: 150 games, minimax vs minimax, seed 300, release, `--no-default-features`, SUM.

The no-return cell's base is the one Liam specified: `--unlimited-spawn on --opening-double off --score-target 5 --capture-to-capturer on`. The references come from #103.

| Cell | n | P1 W-D-L | P2 W-D-L | P1 WR | mean / med turns | opp / self caps per game | mean score P1 / P2 (margin) | end reasons | repetition | comebacks | dest C/I/S/O % | capture C/I/S/O % | lead tail med / ≥10 / loser counterplay | RECOVER/game |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| t5 (2-2-1-1 on, no capturer) | 150 | 75-21-54 | 54-21-75 | 0.500 | 82.3 / 54.5 | 8.15 / 22.91 | 4.21 / 3.95 (abs 1.39) | score 117, draw turn_limit 21, locked_out 7, turn_limit 5 | 16 rep≥3 (avg max 5.08) | 79/129 (61.2%) | 3.0/17.6/59.4/20.1 | 6.7/9.9/72.4/11.0 | 0 / 36 / 37/125 | 0.0 |
| capturer (2-2-1-1 on, target 6) | 150 | 34-20-96 | 96-20-34 | 0.227 | 129.7 / 114.0 | 8.04 / 39.11 | 3.38 / 4.66 (abs 2.21) | score 85, turn_limit 35, draw turn_limit 20, locked_out 10 | 21 rep≥3 (avg max 7.54) | 43/130 (33.1%) | 2.5/19.6/55.2/22.7 | 3.6/15.9/59.4/21.1 | 53 / 96 / 62/127 | 0.0 |
| no-return | 150 | 79-18-53 | 53-18-79 | 0.527 | 113.0 / 92.5 | 6.99 / 37.01 | 3.73 / 3.26 (abs 1.85) | score 96, turn_limit 28, draw turn_limit 18, locked_out 8 | 19 rep≥3 (avg max 7.45) | 61/132 (46.2%) | 2.2/17.3/59.3/21.1 | 2.7/15.7/62.6/19.0 | 24.0 / 86 / 40/130 | 0.0 |

**Caveat.** There is no exact no-return-off twin. Capturer-owns + no 2-2-1-1 + target 5 has not been run without the rule. The `unl_capturer_rep_mm` cell (#105) uses that base with repetition-ends instead, so it is the comparison Liam asked for. It is queued.

## What the rule changes

| Cell | next-turn undo moves | 3-step return cycles per game | tiles parked ≥30 turns per game | suicidal ADDs per game | rep≥3 games | opponent captures of a tile that moved on its owner's last turn |
|---|---|---|---|---|---|---|
| t5 | 184 (1.2 per game) | 1.75 | 3.6 | 13.5 | 16 | 38.6% |
| capturer | 404 (2.7 per game) | 1.56 | 6.2 | 24.6 | 21 | 45.3% |
| no-return | **0** | 1.47 | 5.3 | 23.8 | 19 | **52.4%** |

- **The rule only bites on a few moves.** The bots made a next-turn undo just 1–3 times per game, so banning it moves the stats less than repetition would.
- **Trapping is real but modest.** Just over half of all opponent captures now hit a tile that moved on its owner's previous turn and therefore could not step straight back. That is up from 39–45%. A tile that advances can be caught on its new square, which is the "trap" Liam expected.
- **No sign of permanently stuck tiles.** Parked tiles (sitting 30 or more turns) are fewer than under capturer-owns (5.3 against 6.2). The bots don't wall each other in.
- **Loops are not broken.** 19 games have rep≥3, and their average max repetition is 7.45. The bots switch to other repeating patterns: 3-step A→B→C→A cycles (1.47 per game) and suicidal ADD self-captures used as a tempo pass (23.8 per game). Under capturer-owns, a self-captured tile goes back to your own bench, so that pass is free. This is the main reason for the 37 self-captures per game.

## Snowballs, comebacks, standoff, pace

- **Seats are fair** (P1 0.527), unlike capturer-owns with 2-2-1-1 (0.227).
- **It snowballs, though.** The first +2 holder won 93/99, the lead-tail median is 24 turns (86 games ≥10), comebacks drop to 46% from 61% under t5, and the winner held the center to the end in 43/132.
  - Capturer-owns is the likely cause, not the no-return rule: every capture adds a tile to the capturer's hand. The `capturer_rep` comparison will confirm it.
- **The match-point standoff mostly disappears.** Only 41 games reach a tied (target−1)-all, waiting a median of 11 turns, against 91–102 games and 25–45 turns elsewhere. Games get decided before the standoff because the leader pulls away.
- **Pace is slower:** median 92.5 turns, against 54.5 for t5. The cap is hit in 46 games.

Examples:
- Seeds 10301, 10308 and 10311: 2–3 lead changes each, decided 5-4 in 60–103 turns.
- Seed 300: a 1-1 draw at the cap, with a 142-turn capture drought.

## B + no-immediate-return (`MARK_MODE=b unl_mark_noreturn_mm`)

Base: unlimited spawn, no 2-2-1-1, target 5, graveyard on, mark routing B, plus `--capture-to-capturer on` (routing overrides it).

| Cell | n | P1 W-D-L | P2 W-D-L | P1 WR | mean / med turns | opp / self caps per game | mean score P1 / P2 (margin) | end reasons | repetition | comebacks | dest C/I/S/O % | capture C/I/S/O % | lead tail med / ≥10 / loser counterplay | RECOVER/game |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| mark B | 150 | 65-23-62 | 62-23-65 | 0.433 | 98.1 / 74.5 | 8.08 / 13.93 | 4.03 / 4.05 (abs 1.47) | score 116, draw turn_limit 23, turn_limit 7, locked_out 4 | 8 rep≥3 (avg max 1.85) | 88/127 (69.3%) | 3.1/21.5/52.0/23.4 | 2.6/12.7/68.8/15.8 | 0 / 48 / 34/125 | 14.5 |
| B + no-return | 150 | 69-28-53 | 53-28-69 | 0.460 | 101.7 / 76.0 | 7.98 / 14.81 | 4.09 / 3.89 (abs 1.53) | score 110, draw turn_limit 28, locked_out 7, turn_limit 5 | 7 rep≥3 (avg max 1.60) | 79/122 (64.8%) | 2.7/21.1/52.8/23.4 | 2.3/12.7/70.3/14.7 | 0.0 / 50 / 35/120 | 15.6 |

- **On the best routing base the rule is close to neutral.**
  - P1 wins a bit more (69-28-53 against 65-23-62); the next section shows this is noise. Pace is 1.5 turns slower (median 76 against 74.5). Comebacks drop slightly (64.8% against 69.3%).
  - Loops stay low: 7 games with rep≥3, against 8.
- **Standoff unchanged:** 91 games reach a tied 4-4, waiting a median of 36 turns (28 hit the cap), against 91 / 35 for B alone.
- **Trapping is about the same as B alone.** 54.3% of opponent captures hit a tile that moved on its owner's last turn, against 55.0%. Under B, captured triangle-steals already turn over fast, so the extra commitment barely registers.
- **The capturer-owns snowball seen above doesn't appear here** (first +2 holder won 72/136, lead-tail median 0). That confirms capturer-owns, not this rule, caused the snowball.
- The mark split is unchanged: triangle/dot self-captures 5.9 / 8.9 per game; triangle share of opponent captures 0.57 (winners 0.54, losers 0.59).

## Seat split: noise, not a mechanism

Question: why does P1 go 69-28-53 under B + no-return, against 65-23-62 for B and 64-24-62 for B + repetition-ends? Read from the committed JSONL. Each seed is the same deal (mirror decks) and the same search RNG seed in all three cells.

- **Not significant.** 69 vs 53 decided games gives a two-sided binomial p = 0.17. For B, p = 0.86; for B + rep, p = 0.93.
- **The edge comes from reshuffling, not from particular deals.**
  - The rule changes the search from early on: 142/150 games diverge from plain B, at a median of action 21. Only 4 of those first divergences are an actual banned undo; the other 138 are the search reacting to the changed move set, its own and the opponent's.
  - The result changes on 69/150 deals. Win to win flips: 25 go from P2 to P1 and 15 go from P1 to P2 (p = 0.15). Counting every change of result, 36 favour P1 and 33 favour P2.
- **The ban hits both seats equally.** In plain B, P1 makes 158 next-turn undo moves and P2 makes 162. The 4 games where a banned undo was the first divergence were 3 at P1 and 1 at P2. Results: 2 unchanged, 1 P2 win became a draw, 1 P2 win became a P1 win.
- **The trap and first-capture splits by seat are the same in all three cells.**
  - Strict trap capture means the captured tile was the one its owner moved on their last action, taken on its new node. P1 / P2: B 21.7% / 17.6%, B + no-return 20.2% / 16.5%, B + rep 21.7% / 17.8%.
  - The first opponent capture comes at a median of action 6 in all three cells. P2 makes it more often: 86, 88 and 86 of 150.
  - The first capturer wins about half the time or less in every cell. In B + no-return: P1 32/62, P2 38/88. In B: P1 27/64, P2 34/86.
- **Repetition-ends did not change P1's choices.**
  - All 5 repetition endings (seeds 325, 338, 343, 357, 10305) are move for move identical to plain B up to the ending, and they end with the same outcome B reached at the 200-turn cap.
  - Only 11 games diverge from B at all, at a median of action 95: 7 at P2, 4 at P1. 3 of them changed result: 309 (P1 win to draw), 10303 (P2 win to P1 win), 10321 (P1 win to P2 win). That nets to zero.
- Excerpts:
  - **Seed 10370, the ban as the first divergence.**
    - In B, P2 plays `MOVE 0→8` (t24), P1 plays `2→0` with a self-capture, then P2 moves back `8→0` (t26) and goes on to win 5-4.
    - Under no-return, `8→0` is illegal. P2 plays `ADD →2` instead, and P1 wins 5-4 at t86.
    - This is the one deal where a banned undo flipped a P2 win to a P1 win.
  - **Seed 301, a search-horizon divergence.** No undo is involved.
    - At t19, P1 plays `RECOVER` instead of B's `MOVE 6→0`, because the search sees different replies.
    - P1 wins both games: 5-3 at t58 in B, 4-1 at t49 under no-return.
  - **Seed 343, repetition only shortens the game.**
    - It matches B move for move and ends as a 4-4 repetition draw at t76.
    - Plain B played on to a 4-4 draw at the cap.

Verdict: noise. The rule reseeds most games early and does not give P1 anything systematic. The 16-game gap is within sampling error (p ≈ 0.17) and the per-seat mechanisms are flat. Repetition-ends is almost a no-op on B, so it can't be what evens out the seats there either.

## Take

No-immediate-return does what it says and creates some trap captures, but too few undo moves are banned to fix pace or loops on its own. On the capturer-owns base the game snowballs. On the best routing base (B) it is close to neutral: slightly fairer, slightly slower, and the standoff is unchanged. Repetition-ends (#105) is the direct comparison.
