# Team spawn, matched-neighborhood moves, matched gain, ring budget, mirror palette, surround flip, no bare suicide ADDs

Stacked on `--ruleset capture-b` (#110). Every rule below is a toggle that is off by default. Design discussion is in #111.

**Default capture game: `--ruleset capture-d`** (aliases `capture`, `capture-score`; Liam, t296). It is capture-c plus `--strict-suicide-add on`, `--mark-capture-routing shared` and `--no-return-turns 2`. It plays the committed `cc_all` games exactly (`smoke_cd.jsonl.gz`, seeds 300 and 10300, is identical to `h2h_cc_all_mm` for the same seeds, actions and end included). Shared-graveyard semantics for now: the acting tile's mark routes each removal (CLAIM, Pass and RECOVER count as triangle), RECOVER takes the oldest tile in the shared pile, either player may take it, and the taker owns it.

**`--ruleset capture-c`** is capture-b plus team spawn, `--ring-count-weights 10,3,1`, `--move-support matched` (L1), `--match-gain minus-rings` (L3), `--bench-ring-cap 17`, `--surround-flip on` and `--no-suicide-add on`. It plays the committed `ts_l1_l3_cap17_flip_nosuicide` games exactly and is the baseline for the cc_* cells. The individual toggles stay for ablations, and later flags override the preset. `--ruleset capture-b` is still the old #110 game.

## Team spawn (`--team-spawn on`, `--team-opening-adds 3`)

Each team ADDs only onto its own **spawn diamond**, all game long. The two diamonds sit on opposite sides of the board:

| team | inner (bottom of diamond) | spawn-ring (yellow) | outer (top) |
|---|---|---|---|
| P1 (+Y side) | 6 | 5, 8 | 7 |
| P2 (−Y side) | 18 | 17, 20 | 19 |

- **Construction** (`Board::team_diamond`): take the inner-ring node farthest along +Y (P1) or −Y (P2), add its two spawn-ring neighbors, then add the outer node adjacent to both of those spawns. That gives 4 nodes in a diamond. The picture showed the two yellow spawns and the gray inner node, and the 4th (outer) node sits just above the crop.
- **The global spawn ring is no longer an ADD target.** Its other six nodes are plain board nodes under this rule.
- **Forced opening.** Each player's first 3 actions must be ADDs onto their own diamond, so plies 1–6 are all ADDs. If a player has no legal ADD, the gate lifts.
- **Search sees the gate.** The owed opening ADDs are part of the search hash.
- **Bots unchanged.** The eval's spawn heuristics still read the global spawn ring.

## Matched-neighborhood moves, "MMM" (`--move-support matched|matched-any`)

- **Rule.** A piece may MOVE only if `1 + (matched neighbors) >= n_rings`, counted at its starting node. Singles always move; doubles need 1 matched neighbor and triples need 2.
- **What counts as matched.** A unique graph neighbor whose edge has any match: normal, bead, or braid, the same matches the HUD shows. This counts support (structure), not defense, so it applies even though base capture-b gives normal matches no defense.
- **Whose neighbors.** `matched` counts only your own pieces, which is the supporting structure. `matched-any` also counts enemy pieces. The cells use `matched`.
- **When a side has no legal action.** The existing rule applies (`no_moves_loses`: `LockedOut` loss).

## Matched gain (`--match-gain minus-rings`)

- **Rule.** A piece's defense is `max(rings, matches)`: it is overwhelmed when `mismatch >= max(rings, matches)`. Until its matches exceed its ring count, a piece defends with its ring count alone, and no match type adds anything. Past that point, defense equals the match count. Mismatches sum exactly as before.
- **In the code** this is written as `mismatch − max(0, matches − rings) >= rings`. That's the same inequality, and a unit test checks it exhaustively against `mismatch >= max(rings, matches)`.
- **What counts as a match.** Every channel (normal + bead + braid), summed over the scoring neighbors, the same matches the HUD shows. A neighbor that matches on two channels counts 2.
- **Framing (confirmed by Liam).** In base capture-b (`braid_scoring=braid-only-exact`), only braid matches defend and normal-color matches add nothing, which is intended. L3 replaces that type filter with a threshold: no type adds immediately, and you start adding past your ring count.
- **Intent.** Well-connected singles quickly become as strong as poorly connected doubles and triples. A single with 3 matches defends like an unsupported triple, and singles stay the most mobile pieces.

## Bench ring budget (`--bench-ring-cap 17`)

- **Rule.** The picked 11-tile bench (7-pick plus 4-extend) holds at most 17 rings in total.
- **How the picker enforces it.** The cap applies throughout picking: the 7-pick has to leave room to complete the bench within the cap. A final repair swaps the heaviest bench tile for the lightest reserve if needed.
- **Legal mixes.** Up to 6 doubles, or 1 triple + 4 doubles, or 2 triples + 2 doubles, or 3 triples; singles fill the rest.

## Singles-heavy collections (all cells: `--ring-count-weights 10,3,1`)

- **Generation weights.** Singles:doubles:triples move from 4:3:1 to 10:3:1, so a 27-tile collection averages about 19 singles, 6 doubles and 2 triples instead of 13.5 / 10 / 3.4.
- **Why.** The search picker still prefers heavies (its score weighs rings at 24 points each). Shrinking the heavy supply is what puts singles on the bench.

## Mirror palette (`--mirror-palette permute`)

- **Rule.** Under same-deck mirroring, P2's collection is P1's with a fixed color permutation: Green→Blue→Orange→Green, LavenderPink↔Teal, Maroon fixed (it is the only rare).
- **What stays the same.** Shapes, ring types, dots, marks and rarity tiers.

## Surround flip (`--surround-flip on`)

- **Rule.** After an ADD / MOVE / SWAP / CLAIM resolves (captures first), any tile whose every unique graph neighbor holds a tile of the other owner flips to that owner. It flips even if it matches its neighbors.
- **Board edges are not walls.** An outer node needs all 4 of its neighbors, a spawn-ring node 4, an inner node 3 (center + two spawns), and the center all 8.
- **Flips are simultaneous** and evaluated once per action.
- **Scoring.** A flip is not a capture: no points, nothing leaves the board, and it doesn't count for the self-capture penalty. The tile simply changes sides. Later captures route and score it by its new owner.

## No bare suicide ADDs (`--no-suicide-add on`)

- **Rule.** An ADD is illegal if the added tile would be overwhelmed on arrival and the same ADD captures nothing else. An ADD that loses the added tile is still legal when it also removes at least one other tile, yours or your opponent's.
- **What counts.** Only removals count. A surround flip is not a removal, so an ADD that loses itself and only flips something is still banned.
- **Everything else is unchanged.** Legal ADDs, scoring and recovery work as before. If a player has no legal action left, the existing locked-out rule applies.
- **Bots.** Search uses the same legality check. No tactics were added.

## Strict suicide ADD (`--strict-suicide-add on`)

- **Rule.** An ADD whose added tile would be overwhelmed on arrival is illegal unless the same ADD also removes at least one **enemy** tile. Losing the added tile together with one of your own tiles is banned too, which closes the loophole left by `--no-suicide-add`.
- It works on its own, and it supersedes `--no-suicide-add` when both are on. Search uses the same check. A test on committed games confirms it matches what applying the ADD removes.

## Shared graveyard (`--mark-capture-routing shared`)

- **One graveyard for both players.** It replaces mark routing B's per-player piles.
- **The acting tile's mark decides where each removed tile goes,** as in routing B. The acting tile is the ADDed, MOVEd or SWAP-initiating tile; CLAIM has no acting tile and counts as a triangle.

| acting tile | opponent tile captured | own tile captured |
|---|---|---|
| triangle | shared graveyard | back to your hand (bench) |
| dot (circle) | your hand (you now own it) | shared graveyard |

- **Leaving the graveyard (my choice, open question).** RECOVER (one action) takes the **oldest** tile in the shared graveyard onto the mover's bench, and the mover owns it from then on. Either player may RECOVER whenever the pile is non-empty, so a non-empty pile keeps both sides from being locked out.
- **Logs.** `p1_graveyard` and `p2_graveyard` in the JSONL both report the shared pile's size.

## No return for two turns (`--no-return-turns 2`)

- **Rule.** A tile that left a node by MOVE or SWAP may not be put back on that node by a MOVE or SWAP for the next 2 turns. One turn is one ply, so this covers the opponent's reply and your own next turn.
- **Scope.** It applies to every tile, yours or the opponent's, the SWAP initiator and the displaced partner alike. A flip does not clear the record, and leaving the board does. ADD, CLAIM and RECOVER are never blocked.
- **Relation to older rules.** It stacks with `--no-immediate-return` (the mover only, own next turn) and the existing same-pair SWAP ban (the same two tiles can't swap again right away).

