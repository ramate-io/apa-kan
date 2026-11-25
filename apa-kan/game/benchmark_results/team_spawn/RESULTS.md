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

Cell names: `combo` = `ts_l1_l3_cap17`, `_perm` = permuted mirror deck, `_flip` = surround flip. `capture_b` is the old #110 baseline (global spawns, 4:3:1 collections).

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

## Follow-up: combo + flip + no bare suicide ADDs (`--no-suicide-add on`)

**Rule.** An ADD is illegal when the added tile would be overwhelmed on arrival and the same ADD removes no other tile, yours or your opponent's. A flip is not a removal. Search uses the same check, and no tactics were added. A test replays committed games and confirms the check matches what applying the ADD actually removes.

New cells: `ts_l1_l3_cap17_flip_nosuicide` and `ts_l1_l3_cap17_flip_nosuicide_perm`, with the same settings as the combo cells plus `--surround-flip on --no-suicide-add on`. Both are committed and replayed in CI.

| cell | P1 W-D-L | turns med | TL (draws) | rep draws | locked-out | comebacks | lead≤1 | self-caps/g | enemy caps by rings 1/2/3 | bench singles | single share of actions | life med 1/2/3 | singles alive at end | no-capture turns | longest dry med/p75 | freezes ≥15 mutual/one-sided | flips |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| combo | 66-18-66 | 125.0 | 41 (18) | 4 | 4 | 27/132 | 26 | 10.83 | 488/476/122 | 56.8% | 75.4% | 44/32.5/36.0 | 53.1% | 82.6% | 20.0/34 | 83/31 | 0 |
| combo_flip | 65-18-67 | 116.0 | 32 (17) | 5 | 34 | 31/132 | 31 | 11.69 | 436/456/126 | 56.8% | 70.9% | 40/30/29 | 52.9% | 83.6% | 19.0/37 | 79/31 | 1237 |
| **combo_flip_nosuicide** | 67-9-74 | **106.5** | 15 (**8**) | 4 | 34 | 32/141 | 27 | 9.53 | 456/456/137 | 56.8% | 70.6% | 36.0/26.0/24.5 | 53.1% | 82.6% | 18.0/29 | 75/45 | 892 |
| combo_perm | 73-19-58 | 80.5 | 18 (14) | 12 | 0 | 31/131 | 33 | 19.42 | 512/543/121 | 57.0% | 68.2% | 8.0/16/23.5 | 30.1% | 79.7% | 16.0/30 | 66/34 | 0 |
| combo_flip_perm | 69-17-64 | 84.0 | 18 (14) | 12 | 9 | 33/133 | 25 | 20.01 | 517/520/130 | 57.0% | 67.3% | 8.0/16/23 | 30.2% | 80.2% | 16.5/31 | 67/31 | 94 |
| **combo_flip_nosuicide_perm** | 83-7-60 | 80.5 | 7 (**7**) | 2 | 9 | 43/143 | 22 | 14.94 | 573/510/131 | 57.0% | 65.9% | 9.0/13/17.0 | 28.0% | 78.5% | 15.0/29 | 62/29 | 63 |

**Numbers.**
- **Draws drop by half or more.** Normal decks: 18 draws → 9, turn-limit draws 17 → 8, median 116 → 106.5 plies. Permuted decks: 17 draws → 7, repetition draws 12 → 2.
- **Lockouts are unchanged in count** (34 normal, 9 permuted), but they end harder. Normal-deck lockout losers end with 1 piece in 17 games (8 before) and 0 pieces in 3 (12 before).
- **The permuted cell's P1 edge grows to 83-60.** That's 58% of decisive games, z ≈ 1.9 (p ≈ 0.06). It's borderline, and it was already 73-58 in combo_perm, so it's worth a seat-swap check before trusting it.

**How bots wait out standoffs now** (action mix on turns with no capture available):

| cell | MOVE | SWAP | ADD | RECOVER | SWAP(flip) | MOVE(flip) | suicide ADD |
|---|---|---|---|---|---|---|---|
| combo_flip | 48% | 16% | 16% | 11% | 6% | 1% | 3% |
| combo_flip_nosuicide | 50% | 16% | 18% | 10% | 4% | 1% | – |
| combo_flip_perm | 37% | 8% | 21% | 23% | 0% | 0% | 10% |
| combo_flip_nosuicide_perm | 44% | 9% | 28% | 19% | 0% | 0% | – |

- **The bare-suicide pass is gone.** Bare suicides drop from 450 to 0 (normal) and from 1,275 to 0 (permuted).
- **Bots shift to shuffling MOVEs and safe ADDs.** That works on permuted decks especially, where the suicide pass had been 10% of waiting turns.
- **The rule leaves a loophole.** An ADD that loses the added tile plus one of your own tiles is still legal. Those rise from 112 to 163 (normal) and from 183 to 365 (permuted). It costs two pieces instead of zero, so it's no longer free, but it's the new pass in permuted standoffs. Same-tile repeats within 8 plies fall from 26–35% to 10–13%.

**Flips.**

| cell | flips | games with flips | ping-pong (re-flip within 2 plies) | by SWAP / MOVE / ADD | flipped by a single |
|---|---|---|---|---|---|
| combo_flip | 1237 | 100 | 389 (31%) in 21 games | 965 / 244 / 28 | 92% |
| combo_flip_nosuicide | 892 | 106 | 194 (22%) in 20 games | 659 / 206 / 27 | 91% |
| combo_flip_perm | 94 | 40 | 0 | 41 / 46 / 7 | 68% |
| combo_flip_nosuicide_perm | 63 | 34 | 0 | 22 / 35 / 6 | 71% |

- **Flips spread wider but waste less.** Normal decks: flips show up in slightly more games (106 vs 100), and there are fewer total (892 vs 1237), mostly because ping-pong halves (389 → 194). The side that made most of the flips wins 62 of 106 games.
- **Flip still barely fires with permuted decks,** and the ban doesn't change that. Mismatched contact captures before it can surround.

### Qualitative read of the logs

The same seed gives the same decks in both cells, but games diverge by about ply 3, so pairs share a deck, not a game.

**combo_flip**
- **Flips cluster in long games.** Score wins (78) have a median of 0 flips, and 46 have none at all. Lockouts and turn-limit games have a median of 8, and repetition games 22. Flip kicks in when the game would otherwise freeze.
- **Real counterplay: g10.** It sits in a 4-4 standoff until P1 SWAPs singles in and flips P2's diamond nodes 18, 19 and 20. P2 drops to 1 piece and has no legal action.
- **g45 (lockout at ply 56).** P2 flips P1's inner diamond node 6 (a triple), then node 3. P1's legal actions go 7 → 5 → 2 → 1 → none.
- **Slow squeeze: g16 and g33.** By ply 55 the loser is down to 1–3 pieces with 4–9 legal actions. With nothing left to capture, the score freezes (g33 sits at 4-1 for 50 plies), and the winner shuffles until the last piece can't move. Lockout is the only way such a game can end.
- **Noise: g106 and g52.** g106 is a 100-ply ping-pong. P1 SWAPs a single into the center, which flips node 6. P2 SWAPs back, which flips node 21. This repeats until the turn limit, even while P2 has 4 capture options. g52 has a shorter version (plies 106–116).
- **Singles.** They make 71% of actions and 67% of capturing actions, and they're the actor in 92% of flips. Every SWAP is a single.
- **Center.** It's held by a single 48% of the time and empty 39%. It's the ping-pong pivot: from the center a single touches both inner diamond nodes (6, 18) and the inner ring, so one SWAP can flip a 3-neighbor inner node.
- **Waiting.** Late-game suicide ADDs (g52: P2 at 6–7 legal actions) are a minor share. Most waiting is MOVE shuffling.

**combo_flip_nosuicide**
- **Ping-pong is still there but halved.** g122 has a long center SWAP loop (from ply ~90) flipping nodes 9 and 1, and ends in a 3-3 turn-limit draw. Taking the suicide pass away doesn't stop it. A SWAP in the center is still a free, reversible move.
- **Standoffs now break into lockouts.** g106 (the same deck as combo_flip's 100-ply ping-pong) ends in a lockout at ply 95 at 4-4. P1 flips P2's diamond nodes 3, 21 and 18 with single SWAPs, and P2's legal actions fall 24 → 11 → 4. This happened because P2 no longer has the suicide ADD as a free escape.
- **New, abrupt lockouts on degenerate decks.** g116 is a near-mono-orange deck, all doubles [0,0] plus a few singles, which went to the turn limit under combo_flip. Here it's a lockout at ply 21: one P2 capture and one P1 flip on node 18, then P2's 4 pieces are all frozen by L1, it has no legal ADD, and RECOVER is its only legal action. In the permuted cell, g80 is locked out at ply 25 the same way, after P2 had 2 frozen pieces and only RECOVER for two turns. Lockouts before ply 40 go from 0 to 1 (normal) and from 0 to 2 (permuted).
- **Lockout endings look like conquest.** In 33 of 34 the winner holds the loser's diamond, and the loser usually has exactly 1 piece left (17 of 34).
- **Singles and center are unchanged.** Singles make 71% of actions, 65% of capturing actions and 91% of flips. The center is held by a single 46% of the time.

**Takeaways**
1. The ban does what you hoped on the draw side. It halves draws in both deck types and shortens normal-deck games to about 106 plies. It doesn't kill flip: flips reach more games, with less ping-pong.
2. It doesn't touch the center SWAP ping-pong. A no-immediate-re-flip rule (a ko) or a 4-enemy-neighbor threshold on inner nodes would still help.
3. It makes "frozen + no safe ADD" a lockout. That gives a few very short lockouts on degenerate decks, and lockouts with the loser holding exactly 1 piece become the typical ending. If those are unwanted, a graveyard restructure that gives the stuck side a real move, rather than a free pass, may be the cleaner lever.
4. The own-tile-plus-added-tile suicide is the remaining wait move. It isn't free, but if you want it gone too, ban any ADD where the added tile dies unless it also removes an enemy tile.

## capture-c (new default) + strict ADD / shared graveyard / 2-ply no-return / all three

**Baseline.** `--ruleset capture-c` = combo + flip + `--no-suicide-add` (it reproduces `ts_l1_l3_cap17_flip_nosuicide` game for game). Every cell below is capture-c plus one toggle, 150 games each, mm vs mm, seed 300, release, `--no-default-features`, SUM, with a permuted twin. All of them are committed and replayed in CI.

- `cc_strict`: `--strict-suicide-add on`. An ADD whose added tile dies is illegal unless it also removes an enemy tile.
- `cc_shared`: `--mark-capture-routing shared`. One graveyard. Triangle: opponent captures → shared, self-captures → hand. Dot: self-captures → shared, opponent captures → hand. The acting tile's mark decides; CLAIM counts as triangle. RECOVER takes the oldest shared tile, either player may take it, and the taker owns it.
- `cc_noreturn`: `--no-return-turns 2`. A tile (either side's, swap partners included) can't re-enter a node it left within the last 2 plies.
- `cc_all`: all three.

| cell | P1 W-D-L | turns med | TL (draws) | rep draws | locked-out | comebacks | lead≤1 | self-caps/g | enemy caps by rings 1/2/3 | bench singles | single share of actions | life med 1/2/3 | singles alive at end | no-capture turns | longest dry med/p75 | freezes ≥15 mutual/one-sided | flips |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ts_l1_l3_cap17_flip_nosuicide | 67-9-74 | 106.5 | 15 (8) | 4 | 34 | 32/141 | 27 | 9.53 | 456/456/137 | 56.8% | 70.6% | 36.0/26.0/24.5 | 53.1% | 82.6% | 18.0/29 | 75/45 | 892 |
| cc_strict | 72-6-72 | 89.5 | 16 (6) | 2 | 34 | 26/144 | 24 | 6.95 | 487/401/117 | 57.5% | 73.1% | 23.0/26/30.0 | 47.7% | 82.3% | 15.0/30 | 63/28 | 1070 |
| cc_noreturn | 63-6-81 | 108.0 | 13 (6) | 2 | 30 | 33/144 | 20 | 9.75 | 469/468/139 | 56.8% | 70.1% | 35/27.0/28 | 52.1% | 81.9% | 18.0/29 | 75/37 | 704 |
| cc_shared | 70-11-69 | 113.5 | 23 (10) | 3 | 31 | 33/139 | 25 | 17.12 | 515/430/83 | 56.8% | 68.2% | 24/18/18.0 | 44.7% | 86.7% | 21.0/32 | 92/32 | 1257 |
| cc_all | 73-7-70 | 91.0 | 11 (7) | 0 | 33 | 35/143 | 26 | 9.47 | 502/416/126 | 57.5% | 69.9% | 18.0/22/25.0 | 43.9% | 84.0% | 16.0/27 | 67/31 | 716 |
| ts_l1_l3_cap17_flip_nosuicide_perm | 83-7-60 | 80.5 | 7 (7) | 2 | 9 | 43/143 | 22 | 14.94 | 573/510/131 | 57.0% | 65.9% | 9.0/13/17.0 | 28.0% | 78.5% | 15.0/29 | 62/29 | 63 |
| cc_strict_perm | 86-1-63 | 54.0 | 2 (1) | 0 | 23 | 29/149 | 19 | 8.74 | 550/472/104 | 57.5% | 65.4% | 7/13.0/19 | 26.7% | 73.0% | 9.0/14 | 20/28 | 84 |
| cc_noreturn_perm | 80-8-62 | 81.0 | 8 (8) | 1 | 7 | 39/142 | 20 | 14.96 | 570/522/132 | 57.0% | 65.8% | 9/12/15.0 | 29.1% | 79.0% | 15.0/30 | 66/29 | 82 |
| cc_shared_perm | 79-9-62 | 82.0 | 12 (9) | 3 | 11 | 39/141 | 7 | 24.85 | 651/390/87 | 57.1% | 61.1% | 6.0/8/9 | 24.8% | 82.9% | 14.0/26 | 54/40 | 93 |
| cc_all_perm | 72-2-76 | 60.0 | 3 (2) | 0 | 16 | 34/148 | 11 | 12.14 | 611/418/101 | 57.5% | 64.9% | 6/11.0/18 | 23.6% | 75.9% | 10.0/17 | 30/34 | 60 |

Extra columns (lockouts before ply 40/60; own-only = ADDs where the added tile and only own tiles die; waiting mix = actions on turns with no capture available; graveyard flows = removals by acting mark / victim side / destination, gy = owner's graveyard, opp_gy = opponent's; cross-owner = recovered tiles that started on the other side; 2-ply returns = MOVE/SWAP plies that put a tile back where it was within 2 plies):

| cell | flips | ping-pong re-flips | lockouts (<40/<60 plies) | own-only suicide ADDs | waiting-move mix (no capture available) | graveyard flows (mark,victim,dest) | RECOVER P1/P2 (cross-owner) | 2-ply returns |
|---|---|---|---|---|---|---|---|---|
| ts_l1_l3_cap17_flip_nosuicide | 892 | 194 (22%) | 34 (1/1) | 163 | MOVE 50%, ADD 18%, SWAP 16%, RECOVER 10%, SWAP(flip) 4%, MOVE(flip) 1% | dot/opp→gy 483; dot/own→gy 841; tri/opp→hand 566; tri/own→opp_gy 589 | 808/738 (786) | 182 |
| cc_strict | 1070 | 336 (31%) | 34 (3/7) | 0 | MOVE 50%, ADD 17%, SWAP 17%, RECOVER 8%, SWAP(flip) 6%, MOVE(flip) 1% | dot/opp→gy 439; dot/own→gy 531; tri/opp→hand 566; tri/own→opp_gy 512 | 579/580 (648) | 325 |
| cc_noreturn | 704 | 11 (2%) | 30 (1/1) | 166 | MOVE 51%, ADD 18%, SWAP 17%, RECOVER 10%, SWAP(flip) 3%, MOVE(flip) 1% | dot/opp→gy 498; dot/own→gy 868; tri/opp→hand 578; tri/own→opp_gy 595 | 806/762 (792) | 0 |
| cc_shared | 1257 | 453 (36%) | 31 (0/0) | 571 | MOVE 48%, ADD 24%, SWAP 15%, SWAP(flip) 6%, RECOVER 6%, MOVE(flip) 1% | dot/opp→hand 429; dot/own→shared 491; tri/opp→shared 599; tri/own→hand 2077 | 481/464 (402) | 438 |
| cc_all | 716 | 10 (1%) | 33 (5/6) | 0 | MOVE 52%, ADD 21%, SWAP 17%, RECOVER 6%, SWAP(flip) 4%, MOVE(flip) 1% | dot/opp→hand 472; dot/own→shared 407; tri/opp→shared 572; tri/own→hand 1014 | 408/418 (381) | 0 |
| ts_l1_l3_cap17_flip_nosuicide_perm | 63 | 0 (0%) | 9 (2/2) | 365 | MOVE 44%, ADD 28%, RECOVER 19%, SWAP 9% | dot/opp→gy 535; dot/own→gy 1333; tri/opp→hand 679; tri/own→opp_gy 908 | 1190/1062 (1128) | 2 |
| cc_strict_perm | 84 | 9 (11%) | 23 (8/14) | 0 | MOVE 45%, ADD 29%, RECOVER 17%, SWAP 9%, SWAP(flip) 1% | dot/opp→gy 477; dot/own→gy 653; tri/opp→hand 649; tri/own→opp_gy 658 | 694/689 (785) | 10 |
| cc_noreturn_perm | 82 | 0 (0%) | 7 (2/2) | 374 | MOVE 44%, ADD 27%, RECOVER 19%, SWAP 10% | dot/opp→gy 531; dot/own→gy 1354; tri/opp→hand 693; tri/own→opp_gy 890 | 1182/1082 (1133) | 0 |
| cc_shared_perm | 93 | 12 (13%) | 11 (2/3) | 1045 | MOVE 42%, ADD 40%, RECOVER 9%, SWAP 8% | dot/opp→hand 505; dot/own→shared 639; tri/opp→shared 623; tri/own→hand 3089 | 524/545 (454) | 13 |
| cc_all_perm | 60 | 0 (0%) | 16 (7/9) | 0 | MOVE 45%, ADD 34%, RECOVER 11%, SWAP 10% | dot/opp→hand 465; dot/own→shared 507; tri/opp→shared 665; tri/own→hand 1314 | 466/479 (416) | 0 |

**Numbers.**
- **Strict ADD** removes the last suicide wait move (own-only 163 → 0, 365 → 0), and games get shorter (normal median 106.5 → 89.5, permuted 80.5 → 54). The cost is very short lockouts: before ply 40 they go 1 → 3 (normal) and 2 → 8 (permuted). Permuted lockouts go 9 → 23, and the P1 edge stays (86-63).
- **2-ply no-return** is the ping-pong fix. Re-flips within 2 plies fall from 194 (22% of flips) to 11 (2%), and 2-ply returns drop from 182 to 0. Everything else stays close to the default: median 108, 6 draws, 30 lockouts, and no new short lockouts. Permuted decks are unaffected, since they never ping-ponged. Normal decks went 63-81 for P2 (p ≈ 0.15), likely noise.
- **Shared graveyard** on its own is the weakest cell: 11 draws and 23 turn-limit games (default 9/15), median 113.5, and self-captures per game 9.5 → 17.1 (14.9 → 24.9 permuted). Triangle self-captures now go straight back to hand (2,077 and 3,089 removals), so self-capture is free again: own-only suicide ADDs come back (163 → 571, 365 → 1,045), 95% of those tiles are back on the bench next ply, and ADD rises to 40% of permuted waiting moves. Ping-pong also gets worse (36% of flips).
- **All three** is the best-balanced cell: 73-7-70 and 72-2-76, 7 and 2 draws, median 91 and 60, ping-pong 1% and 0%, and no own-only suicides. Strict closes the free self-capture that shared opens, and no-return closes the center loop. It keeps strict's short-lockout cost (5 and 7 before ply 40). 12 of the default's 15 turn-limit games are decided (7 lockouts, 5 score wins), and 3 still go to the turn limit.
- **Graveyard use.** Under shared, the pile stays small (median 2 at the end, max 4 in a game), and RECOVERs drop by about 40% (1,546 → 945 normal) because triangles' own losses skip the graveyard. Both players draw from the pile about equally (481/464), and 402 of 945 recovered tiles started on the other side.

### Qualitative read of the logs
- **Strict: short lockouts are "frozen + no safe ADD".** cc_strict_perm g20 (seed 310) is locked at ply 9. P1's single trades itself for one of P2's (ply 6). P2's 2 diamond tiles are frozen by L1, and every diamond ADD would die without taking an enemy, so its only action is RECOVER (ply 7), and then it has nothing. cc_strict g72 and g138 lock at ply 15 the same way. Under the default, those seeds are a 39-, 63- and 128-ply score game. The free suicide was also the stuck side's escape valve.
- **No-return: the center loop is gone and standoffs resolve.** g122 under the default has 38 center re-flips and ends in a 3-3 turn-limit draw. Under no-return, P2's singles keep flipping without the swap-back, P2 grows to 12 pieces, and P1 is locked out at ply 106. Default g0 (33 re-flips, repetition at 196) becomes a lockout at 95, g21 (repetition at 75) a score win at 142, and g133 (turn limit) a score win at 192. A 2-ply window is enough: the 3-tile center rotation needs the swap-back that this rule bans.
- **Shared: free trades and shuffles.** cc_shared g131 is a 79-re-flip center SWAP loop (nodes 6 and 21, plies 145–160+) that ends at the turn limit. g121 is a 30-ply double-vs-double MOVE shuffle around a held center. cc_shared_perm's wait move is a triangle ADD that kills itself plus an own neighbour, with both returning to hand next ply (902 of 1,045).
- **All three: fast, trade-heavy games.** cc_all_perm g71 (seed 10335) is a chain of 1-for-1 kamikaze singles (plies 6, 14, 17, 23 each capture an enemy and lose the attacker), which is cheap because the triangle attacker comes back to hand. It ends in a 3-3 lockout at ply 29. cc_all g90 (seed 345) locks at ply 9: P2 is down to 2 frozen tiles with no strict-legal ADD, and P1 takes the only tile in the shared pile with its own RECOVER. Under the default, that seed is a 79-ply score game. On normal decks, lockouts in cc_all still look like conquest (the winner holds part of the loser's diamond in 29 of 33). On permuted decks they mostly don't (2 of 16), and the loser usually still has 2 frozen pieces.

### Takeaways
1. **Keep 2-ply no-return.** It is cheap and targeted: it kills the ping-pong and leaves everything else alone.
2. **Shared graveyard needs strict ADD** (or another way to make self-capture cost something). On its own it reopens the free pass through "triangle self-capture → hand".
3. **All three** gives the most decisive, balanced games, but it makes "frozen + no strict-legal ADD + empty pile" a very fast lockout. If that's unwanted, a small valve would help without a free pass: always allow an ADD to an empty diamond node, or count a frozen side's RECOVER as a turn even with an empty pile (pass), or let a stuck player swap two of its own frozen tiles.

### Open questions
- **How tiles leave the shared graveyard.** Currently RECOVER takes the oldest tile, either player may take it, and the taker owns it. Alternatives: only the side that lost the tile may recover it; the mover chooses any tile; or newest first.
- **Which mark routes a removal.** Currently it's the acting tile's mark (CLAIM, Pass and RECOVER count as triangle). The alternative is the captured tile's own mark.
- **Should a triangle's self-capture going to hand be free?** It's the source of the shared-graveyard free pass. It could go to the shared pile instead, or cost a turn.
- **No-return window.** 2 plies is enough in the logs. 4 plies would also stop longer rotations, but we haven't seen one survive 2.
- **The permuted-deck P1 edge persists** in strict (86-63) and shared (79-62), but not in all three (72-76). A seat-swap check is still pending.
