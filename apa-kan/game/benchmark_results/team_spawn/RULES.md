# Team spawn, matched-neighborhood moves, matched gain, ring budget, mirror palette, surround flip

Stacked on `--ruleset capture-b` (#110). Every rule below is a toggle that is off by default. Design discussion is in #111.

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
- **What counts as matched.** A unique graph neighbor whose edge has any match: normal, bead, or braid, the same matches the HUD shows. This holds even though live capture-b scoring (`braid-only-exact`) gives 0 defense for a normal-color match.
- **Whose neighbors.** `matched` counts only your own pieces, which is the supporting structure. `matched-any` also counts enemy pieces. The cells use `matched`.
- **When a side has no legal action.** The existing rule applies (`no_moves_loses`: `LockedOut` loss).

## Matched gain (`--match-gain minus-rings`)

- **Rule.** Overwhelm when `mismatch − max(0, matches − n_rings) >= n_rings`. Mismatches sum exactly as before.
- **What counts as a match.** Every channel's match (normal + bead + braid). Under the live braid-only scoring, today only braid matches defend. With this toggle, normal matches defend too, but only those beyond the ring count. So a single with 3 matches gets +2, and a triple needs 4 matches before it gains anything.

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
