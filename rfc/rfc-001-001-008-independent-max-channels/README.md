# RFC 001-001-008: Independent-max edge totals

Experimental default on this branch. Stacked on the character-library work.
`--channel-mode additive` restores RFC 001-001-002 additive stacking.

## Problem

Single beads felt worthless, but they are only worthless as a **lone** ring
(a 1-bead vs a 2-ring is +1, so the 2-ring survives). On a mixed tile a bead
can force a productive mismatch while another channel matches. Additive
stacking across four channels is also hard to compute by hand; the game
should be playable over the board.

## Accepted default on this branch

`--channel-mode independent-max`:

1. **Independent channels.** Normal, zag, bead, and braid each keep their
   own component math. There is no shared bead/normal channel.
2. **Max match / max mismatch.** For each undirected edge, the totals used
   by overwhelm, HUD `tile_score`, capture logging, and search are

   ```
   match    = max(normal_match, bead_match, braid_match)   // zag never matches
   mismatch = max(normal_mismatch, bead_mismatch, zag_mismatch, braid_mismatch)
   signed   = mismatch − match
   ```

   Tile-level `effective` is still the **sum of those per-edge signed
   values** over `Board::incident_others` (inner↔center is two petals).
3. **Generation.** Braids are the only heavy group (existing piece-level
   2–3 `braid_numer / 7` roll). Remaining light slots elect
   `normal : zag : bead = 5 : 3 : 1`. Single-bead tiles are allowed.
   Emblems stay None 200 / Crescent 1 / Zs 1 / Blocker 1 / Rosette 1.

`--channel-mode additive` keeps RFC 002 stacking (`--bead-scoring` then
zag+braid add) and 2–3 bead groups.

## Rule mapping (current totals → max)

Channel *components* (`Game::edge_channels`) are unchanged. Only
aggregation changes. `--bead-scoring` is ignored under independent-max.

| Rule / consumer | Additive (RFC 002, `--channel-mode additive`) | Independent-max (default) |
|---|---|---|
| Normal | match 1 or hitch 1 | same component; may lose the edge max to another channel |
| Zag | `zag_mismatch` sum/max of strokes, then **added** | same component; edge mismatch is `max(..., zag)` |
| Braid match | leftover normal **stacks** (`3` for braid 2 + normal 1) | `max(braid, normal)` (`2` in that example) |
| Failed braid `one` | +1 **plus** other mismatches | `max(1, other mismatches)` |
| Failed braid `weight` / `sum` | research variants, still add | still compute the component; edge takes the max |
| Bead | shared: `max(normal, bead)` then zag+braid add; independent: sum all | four independent components, then max |
| Two Zag Zs | +1 on the zag component before aggregate | same; only the aggregate changes |
| Crescent | `eff > n_rings` at 1 occupied neighbor | same test; `eff` uses max-per-edge |
| Overwhelm | `Σ(mismatch − match) >= weight` | same inequality, new totals |
| HUD `tile_score` | `(weight + Σ match, Σ mismatch)` | same shape, per-edge maxes |
| `classify_bead_capture` | with vs without bead fields, then aggregate | same, through max |
| Lane paint | every firing channel | **unchanged** (visuals still show all lanes) |

### Worked edges (simplest hand reading)

Signs: `(match, mismatch)`, signed = mismatch − match.

| Case | Components | Additive shared | Independent-max |
|---|---|---|---|
| Orange zag vs orange normal | N hitch 1, Z 1 | (0, 2) | (0, 1) |
| 1 blue bead vs 2-ring hitch | N hitch 1, Be 1 | (0, 1) | (0, 1) |
| Orange normal + blue bead vs orange | N match 1, Be unmatched 1 | (1, 1) | (1, 1) |
| 3 orange beads vs 3-braid fail | Be 3, Br fail 1 | (0, 4) | (0, 3) |
| Braid 2 + leftover normal | N 1, Br 2 | (3, 0) | (2, 0) |
| Zag-on-zag + leftover hitch | N hitch 1, Z ≥ 1 | hitch + zag | max(hitch, zag) |

A lone 1-bead vs a 2-ring is still only +1 under both defaults (the 2-ring
survives). The productive case is a bead **plus another ring**.

## Generation

Live `sample_tile_with` on this branch (CorePass is still RFC-only):

1. Color tree / ring count unchanged.
2. Piece-level braid (`braid_numer / 7`, size 2 or 3, padding). Only heavy type.
3. Each remaining **normal** slot: weighted draw 5:3:1 normal/zag/bead.
   `--zags off` / `--beads off` drop that weight and skip its RNG.
   Non-default `--zag-chance` / `--bead-chance` replace that type's weight
   so collectors stay one knob (`--p1-deck bead` → bead weight 5).

`CorePass::base()` rarity chart (when wired) matches that light table:
`light_weights = [5, 3, 1]`, heavy = braid only. See
[`rfc-000-000-00n-generation`](../rfc-000-000-00n-generation/README.md).

Bead-heavy quota no longer forbids 1-bead tiles or requires a 3-bead tile
(independent light rolls make triples rare). It still asks for ≥10 bead
rings and ≥2 stacked (2+) bead tiles. Custom TOML may list a single bead.

## Open design questions

1. **Gapped CorePass heavies.** Live generation never emits a gapped braid
   (the piece-level roll occupies the first 2–3 slots). The RFC CorePass
   previously treated gapped heavies as beads. Simplest hand reading once
   beads are light: gapped / lone heavy coins fall through to the 5:3:1
   light election (they can become a single bead). Consecutive ≥2 heavies
   stay a braid. Not wired; CorePass is still RFC-only on this stack.
2. **`--zag-chance` / `--bead-chance` stuffing.** Independent-max maps
   default `1` onto weights 3 and 1, and any other positive numerator onto
   that weight directly. That is simpler than a 7-denominator coin, but it
   is not the same as additive-mode `k/7`.
3. **`--beads off` seed match.** Additive mode skipped a later bead roll so
   the deal matched pre-bead. Independent-max elects zag and bead jointly,
   so `--beads off` changes the zag/normal split (5:3:0 vs 5:3:1). Accepted
   here; `--zags off --beads off` still skips light RNG after braid.
4. **Whether HUD / lane paint should hide non-winning channels.** Chosen:
   paint every firing channel; only the numeric totals use max.
5. **Bead collector triples.** Dropped the ≥1 triple quota so bead-heavy
   27-tile bags still form under 5:3:5 light stuffing. Revisit if collectors
   should force a 3-bead tile some other way.

## Tests

- `independent_max_single_bead_plus_normal_is_a_productive_mismatch`
- `independent_max_zag_on_zag_takes_the_channel_max`
- `independent_max_braid_does_not_stack_with_leftover_normal`
- `independent_max_mixed_bead_zag_braid_takes_per_kind_maxima`
- `independent_max_light_election_is_five_three_one` (n=20000)

RFC 002 additive table tests pin `--channel-mode additive`.
