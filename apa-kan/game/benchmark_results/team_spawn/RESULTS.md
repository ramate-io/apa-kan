## Results: team spawn + singles-heavy collections, L1 / L3 / combo, permuted mirror decks, surround flip

Setup for every cell: 150 games, minimax vs minimax, seed 300, release, `--no-default-features`. Rules are `--ruleset capture-b --team-spawn on --ring-count-weights 10,3,1` plus each cell's toggles. Data and the rule definitions are in #112 (`apa-kan/game/benchmark_results/team_spawn/`, `RULES.md`). All cells replay in CI.

**Rule definitions as run**
- **Spawn diamonds.** P1 is {5, 6, 7, 8}, the +Y side: inner node 6, spawn-ring nodes 5 and 8, outer node 7. P2 is {17, 18, 19, 20}, the opposite side: inner 18, spawns 17 and 20, outer 19.
  - Each player's first 3 actions are ADDs onto their own diamond.
  - After that, a team ADDs only onto its own diamond.
  - Enemy pieces may stand on your diamond.
- **L1 (MMM).** A MOVE needs `1 + friendly matched neighbors >= rings`, checked at the start square. Any match type counts.
- **L3.** Defense = `max(rings, matches)`, so a piece is captured when `mismatch >= max(rings, matches)`.
  - Matches from every channel count, but no match type adds anything until the total passes the ring count.
  - This replaces base capture-b's braid-only defense (which is intended for the baseline) with a threshold.
  - The code's `mismatch − max(0, matches − rings) >= rings` is the same inequality, and a unit test checks it exhaustively.
- **Combo.** L1 + L3 + a 17-ring bench cap.
- **Perm.** P2's mirrored deck uses permuted colors: G→B→O→G, Lavender↔Teal, Maroon fixed.
- **Flip.** A tile flips owner when every graph neighbor holds an enemy tile.
  - Board edges don't count as walls, so an inner node needs 3 enemy neighbors, spawn and outer nodes need 4, and the center needs 8.
  - A flip scores nothing.

### Aggregate

- **comebacks:** the eventual winner trailed by 2 or more points at some point, out of decisive games.
- **lead≤1:** the score lead never exceeded 1.
- **freezes:** a side went 15 or more own turns with no capture available, counted per game (mutual = both sides).
- **self-caps:** own pieces removed by your own action. This is mostly the suicide ADD described below.

| cell | P1 W-D-L | turns med | TL (draws) | rep draws | locked-out | comebacks | lead≤1 | self-caps/g | enemy caps by rings 1/2/3 | bench singles | single share of actions | life med 1/2/3 | singles alive at end | no-capture turns | longest dry med/p75 | freezes ≥15 mutual/one-sided | flips |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| capture_b | 69-28-53 | 74.0 | 30 (26) | 4 | 7 | 42/122 | 14 | 13.7 | 98/656/442 | 11.3% | 5.1% | 4/12.0/17 | 25.7% | 84.9% | 16.0/38 | 65/24 | 0 |
| ts_base | 67-36-47 | 141.0 | 47 (35) | 4 | 0 | 32/114 | 19 | 17.09 | 337/514/332 | 38.4% | 33.1% | 31.0/25/23.0 | 45.2% | 84.8% | 28.0/51 | 95/33 | 0 |
| ts_l1 | 63-11-76 | 102.0 | 19 (10) | 8 | 8 | 27/139 | 25 | 19.05 | 383/478/218 | 38.4% | 54.1% | 27/24.0/33.0 | 44.1% | 81.2% | 18.0/30 | 73/34 | 0 |
| ts_l3 | 58-33-59 | 150.0 | 53 (32) | 4 | 2 | 30/117 | 28 | 14.04 | 324/534/321 | 38.4% | 38.5% | 42.0/32.0/24.0 | 53.1% | 84.3% | 28.0/50 | 102/24 | 0 |
| ts_l1_l3_cap17 | 66-18-66 | 125.0 | 41 (18) | 4 | 4 | 27/132 | 26 | 10.83 | 488/476/122 | 56.8% | 75.4% | 44/32.5/36.0 | 53.1% | 82.6% | 20.0/34 | 83/31 | 0 |
| ts_l1_l3_cap17_flip | 65-18-67 | 116.0 | 32 (17) | 5 | 34 | 31/132 | 31 | 11.69 | 436/456/126 | 56.8% | 70.9% | 40/30/29 | 52.9% | 83.6% | 19.0/37 | 79/31 | 1237 |
| ts_base_perm | 58-44-48 | 107.0 | 37 (37) | 10 | 1 | 28/106 | 25 | 21.49 | 304/553/365 | 39.8% | 26.2% | 8/13/17 | 32.2% | 85.0% | 24.5/64 | 87/19 | 0 |
| ts_l1_perm | 73-17-60 | 84.0 | 25 (16) | 17 | 4 | 24/133 | 22 | 26.13 | 370/486/226 | 39.8% | 57.2% | 8.0/14.0/23 | 29.3% | 81.1% | 18.0/39 | 69/36 | 0 |
| ts_l3_perm | 57-30-63 | 106.0 | 26 (25) | 9 | 1 | 38/120 | 23 | 17.5 | 336/541/368 | 39.8% | 30.0% | 10.0/14.0/17.0 | 37.6% | 85.3% | 24.0/52 | 88/33 | 0 |
| ts_l1_l3_cap17_perm | 73-19-58 | 80.5 | 18 (14) | 12 | 0 | 31/131 | 33 | 19.42 | 512/543/121 | 57.0% | 68.2% | 8.0/16/23.5 | 30.1% | 79.7% | 16.0/30 | 66/34 | 0 |
| ts_l1_l3_cap17_flip_perm | 69-17-64 | 84.0 | 18 (14) | 12 | 9 | 33/133 | 25 | 20.01 | 517/520/130 | 57.0% | 67.3% | 8.0/16/23 | 30.2% | 80.2% | 16.5/31 | 67/31 | 94 |

| cell | suicide ADDs | enemy on mover's diamond | first capture ply (median) | captures inside victim diamond | games reaching 4-4 (TL draws) |
|---|---|---|---|---|---|
| ts_base | 19% | 21% of plies | 11 | 40% | 90 (33) |
| ts_l1 | 27% | 17% | 13 | 34% | 52 (5) |
| ts_l3 | 12% | 27% | 11 | 38% | 91 (27) |
| **combo** | 9% | 31% | 13 | 30% | 59 (14) |
| **combo_flip** | 13% | 18% | 12 | 34% | 51 (16) |
| ts_base_perm | 27% | 10% | 7 | 54% | 107 (37) |
| ts_l1_perm | 38% | 6% | 8 | 51% | 60 (16) |
| ts_l3_perm | 18% | 11% | 7 | 53% | 104 (25) |
| **combo_perm** | 27% | 8% | 7 | 53% | 83 (12) |
| **combo_flip_perm** | 29% | 8% | 7 | 54% | 75 (12) |

Old capture-b (global spawns, 4:3:1 collections): first capture at a median of ply 5, singles made 8% of captures, 11% of bench slots were singles.

### The combo (L1 + L3 + cap 17), the candidate

- **Normal decks: 66-18-66, median 125 plies.** It's balanced.
  - Singles fill 57% of bench slots, make 75% of piece actions and 68% of enemy captures, live a median of 44 plies (against 4 in old capture-b), and 53% are still alive at the end.
  - When a single makes a capture, it survives 25% of the time (175 of 710). In old capture-b that was 1 of 92.
- **L1 makes heavies into structures.** Triples are frozen 86% of the time and doubles 39%. An unsupported double is captured about 2× as often per exposed ply as a supported one, which is your "cut them off for free points".
- **Stalls are reduced, not gone.** 18 turn-limit draws (35 in the team-spawn baseline). 59 games reach 4-4, against 90 in the baseline. 83 of 150 games still have a mutual dry spell of 15 or more turns.
- **Qualitative read (several games).** Singles sit in the center 55% of plies, and center sitting is now a singles job; in the old capture-b it was a triples job.
  - Heavies stay home on the diamond, and singles raid.
  - Enemy pieces squat on the opponent's diamond 31% of the time. That blocks ADDs and makes ADDs there die on arrival.

### Permuted mirror decks (combo_perm)

- **Seat balance gets worse: 73-19-58.** Three of the four perm cells lean toward P1 (base 58-48, L1 73-60, combo 73-58), and L3 leans P2 (57-63). A 15-game gap over about 131 decisive games is roughly 1.3σ, so I'd re-run with more seeds before trusting the lean.
- **Games get short and violent: median 80.5 plies,** which is chess length.
  - The first capture comes at a median of ply 7, and 75% of games have one by ply 10.
  - More than half of all captures happen inside the victim's diamond.
  - With no shared palette, any contact between the two sides is mostly mismatch. So the two diamonds, whose inner nodes are 2 steps apart through the center, become a kill zone.
- **Singles become fodder again under perm.** Median life drops from 44 to 8 plies, and a capturing single survives 8% of the time (59 of 764).
  - L3's defense only helps a piece with many matching neighbors, and enemy neighbors under perm almost never match.
  - So L3 is mostly a same-palette tool, and the spawn spacing matters much more with permuted decks.
- **Squatting disappears** (8% of plies), because a squatter in an enemy-colored neighborhood dies.

### Surround flip (combo_flip, combo_flip_perm)

- **Normal decks: 1237 flips in 100 of 150 games.** Results 65-18-67, median 116 plies (from 125), 17 turn-limit draws.
  - Locked-out endings jump from 4 to 34: flips strip one side's material until L1 leaves it no legal action. Game 16, for example, ends with P2 holding one piece.
  - 77% of flips hit singles. 76% hit the inner nodes, which have only 3 neighbors, and nodes 6 and 18, the inner node of each diamond, are the top two.
  - 965 of the 1237 flips come from a SWAP.
  - **There's a ping-pong.** In game 0, plies 22–35, the two sides alternately SWAP a single into the center and flip node 6, then node 18, every ply. 248 flips were reversed within 10 plies.
- **Permuted decks: only 94 flips in 40 games, and no change in draws** (14 in both cells).
  - A tile surrounded by enemy colors is normally captured before it can be flipped. Flips only happen where the colors happen to match.
  - Results 69-17-64, median 84 plies. Locked-out endings rise from 0 to 9.
- **Takeaway.** Flip does what you intended where colors match, giving counterplay against matched walls. But as defined it is too cheap on 3-neighbor inner nodes, and it needs an anti-ping-pong rule.

### Suicide ADD as a free pass (all cells)

- **The pattern.** A bot ADDs a piece into a spot where it is immediately overwhelmed and nothing else is captured. With the self-capture penalty off, that costs nothing, the piece comes back via RECOVER, and the turn is effectively a pass.
- **How common.** These ADDs are 9–38% of all ADDs. 78–94% are made when no capture is available, and 19–40% of the time the same tile repeats it within 8 plies.
- **Where it matters.** Bots use it to wait out standoffs, and it is a big part of the "self-caps per game" column.
- **The rule-level fix** is either the #110 self-capture penalty or making an ADD that is overwhelmed on arrival illegal. It's not a bot fix.

### Suggestions

1. **Keep the combo as the base.** For permuted decks, move the diamonds apart. For example, drop the inner nodes 6 and 18, which are 2 steps apart through the center, and spawn on 5, 7, 8 and 17, 19, 20 only, or shift each diamond one ring outward. That should restore the build-up time team spawn was meant to give. I can run combo and combo_perm with that.
2. **If surround flip stays,** two rule-level changes:
   - Require at least 4 enemy neighbors, or exclude the center as a surrounding neighbor, so 3-neighbor inner nodes aren't free.
   - Add a ko rule: a tile flipped on the previous ply can't be flipped back on the next one.
3. **Rulesets where flip would likely help, from the logs:**
   - **Same-palette and mono-color decks**, where matches freeze contact: the #111 freeze cases and the 4-4 standoff seeds such as 341. That's where it produced the most flips.
   - **The control games** (control cascade / majority). A flip converts control directly, without needing a mismatch, which is the missing counterplay #111 found there too.
   - **Little help with permuted or random-palette decks**, where contact already kills.
4. **Turn the self-capture penalty on with the combo,** or ban dead-on-arrival ADDs, to remove the free pass.
