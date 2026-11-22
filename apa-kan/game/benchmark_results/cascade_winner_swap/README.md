# Cascade-winner extra CONVERT + center mismatch

Stacked on PR #85 (`exactplay` bots) / PR #83 (live playtest `RuleSet`).
Two **off-by-default** rule toggles. Live defaults are unchanged:
braid-only exact, single braids, additive channels, double-counted
center↔inner, ring weights 4:3:1, color drift 160, same-deck 27-tile
start, adjacent normal-only single SWAP with no swap-back,
`cascade_turn_gain` already on.

Liam wants cascades/center fights to terminate faster and give the
player who **entered with more material** a stronger advantage, without
turn-count tracking over the board. Correction after the first reading
of this PR: extra-turn “swap” means **conversion** — “swap the piece to
your team. Basically, turn it over.” The piece stays on its node.

Reproduce with [`run_h2h.sh`](run_h2h.sh). Survey:
[`survey.py`](survey.py) (reuses [`../exactplay/survey.py`](../exactplay/survey.py);
first center *take* still counts only if the landing piece is there
after captures). Quote numbers from the committed `*.txt` / JSONL in
this directory. P1 win-rate CI is Wilson 95% on wins/n.

## What the flags do

| Flag | Default | Effect |
|---|---|---|
| `--cascade-winner-swap on\|off` | off | Extra turn when the **pre-action** center occupant is among this action’s removals. Leader is pre-action `total_material`, tie-break spawn-token count (“capture hold”). Tied material **and** tokens: no award. |
| `--extra-turn-action convert\|swap` | `convert` | Extra-turn **CONVERT** (ownership flip, piece stays) or permissive **position-exchange SWAP**. Conversion is the benched reading. |
| `--extra-convert-mode any\|not-center\|inner` | `any` | CONVERT target filter. `any` = any opponent piece including the origin. Cheap variants wired, not benched. |
| `--extra-swap-mode any\|own\|not-center` | `any` | Partner filter for extra-turn SWAP when `--extra-turn-action swap`. |
| `--center-mismatch on\|off` | off | Unmatched center occupant scores a synthetic **+1 mismatch**. |

Missing JSONL: `cascade_winner_swap` / `center_mismatch` replay as off;
`extra_turn_action` as convert; convert/swap modes as `any`. Older logs
from the first (position-exchange) reading of this PR replay as
**convert** if `extra_turn_action` is absent — those SWAP logs were
replaced.

### CONVERT (the benched extra)

`Action::Convert { node }` flips the occupant’s owner to the extra-taker.
The tile id, rings, and node stay put. Legal only on an extra turn
while `cascade_winner_swap` is on and `extra_turn_action=convert`.
Initiator is not required; any opponent piece matching
`extra_convert_mode` is a target.

Local-claim captures then fire on that node and its neighbors, so a
stolen unmatched 1-ring *can* self-pop. Across these cells
`self_pop=0` and CONVERT itself never removed an opponent tile
(`opp_caps=0`): bots steal 2-rings and 3-rings.

HUD: `+turn convert-any`. Click an opponent piece on an extra turn
issues CONVERT (green highlight). Extra turns still allow ADD / MOVE /
CLAIM / live SWAP; CONVERT is an extra verb, not a CONVERT-only turn.

Fight-gain extras and live `cascade_turn_gain` extras both unlock
CONVERT while the flag is on. Same-action fight+cascade still banks
only one extra (`# fight_gain` still counts the fight).

### Position-exchange SWAP (kept, not benched)

`--extra-turn-action swap` restores the first reading: extra-turn
`Action::Swap` as a looser position exchange (any own initiator, any
occupied partner, not necessarily adjacent). See the previous commit
on this branch for that package. These cells all use `convert`.

### Center mismatch

“Matched” means some neighbor edge has a **HUD** normal/braid/bead
match > 0, even when braid-only-exact scores that match as 0.
Mismatches alone do not clear the penalty. Empty-center unmatched
1-rings already self-pop; the +1 is what newly overwhelms them under
this flag. Unmatched **2-rings** (weight 3) get +1 and stay.

## Package

Release `apa-kan-bench`, `--no-default-features`, `--budget
nodes:33000,iters:650` (search-pick stays on this light budget at 4x;
in-game overlay `--p1-budget` / `--p2-budget`), `--games 50` (light) or
`--games 10` (4x), `--jobs 4`, `--no-depth-probe`, `--zags on`,
`--zag-mismatch sum`, `--endgame-rule control-cascade`,
`--cascade-turn-gain on`, `--capture-mode local-claim`,
`--no-same-piece-twice off`, `--pick search`, `--pairings
minimax:mcts,mcts:minimax`, `--same-deck on`, `--seed 1`,
`--search-profile exactplay`. 50×2 pairings = **100 games** light;
10×2 = **20 games** 4x (`nodes:132000,iters:2600` both seats).

| Log | Flags | Games | Budget |
|---|---|---|---|
| [`h2h_baseline`](h2h_baseline.txt) | both off (live) | 100 | light |
| [`h2h_r1`](h2h_r1.txt) | `--cascade-winner-swap on` (convert/any) | 100 | light |
| [`h2h_r2`](h2h_r2.txt) | `--center-mismatch on` | 100 | light |
| [`h2h_both`](h2h_both.txt) | convert + center +1 | 100 | light |
| [`h2h_deep4x_baseline`](h2h_deep4x_baseline.txt) | both off | 20 | 4x |
| [`h2h_deep4x_r1`](h2h_deep4x_r1.txt) | convert extra | 20 | 4x |

Light baseline is a **byte-level replay** of PR #85
[`exactplay/h2h_self`](../exactplay/h2h_self.txt): P1 **48-2-50**, mean
**58.41**, lockout 24, control 71, turn_limit 1. Light r2 is the same
games as the first reading of this PR (center +1 does not touch
CONVERT).

**Best combo for the 4x check:** `r1` (conversion only). It shortened
mean **and** median more than `both` (50.49 / 34 vs 53.07 / 34 vs
baseline 58.41 / 43). `both` used CONVERT more (0.92 vs 0.67/game) but
pushed lockouts higher (46 vs 40). Rule 2 alone lengthened games and
raised never-left.

## Light 100-game exactplay self-play

Quoted from the `*.txt` logs plus [`survey.py`](survey.py) on the JSONL
(median / never-left / sit-and-win / vacates / extra-turn use /
leader-after-first-award). Wilson 95% CI on P1 wins / 100.

| | baseline | r1 (convert extra) | r2 (center +1) | both |
|---|---|---|---|---|
| P1 W-D-L | **48-2-50** | **56-3-41** | **49-2-49** | **52-2-46** |
| P1 win CI | 48.0% [38.5, 57.7] | 56.0% [46.2, 65.3] | 49.0% [39.4, 58.7] | 52.0% [42.3, 61.5] |
| mean / median turns | 58.41 / 43 | **50.49 / 34** | 62.33 / 46 | 53.07 / 34 |
| p90 | 128 | **105** | 135 | 137 |
| 200-turn draws | 1 | 3 | 2 | 2 |
| lockouts | 24 | **40** | 22 | **46** |
| control | 71 | 49 | 71 | 45 |
| never-left | 23/93 (24.7%) | **16/91 (17.6%)** | **30/90 (33.3%)** | 16/92 (17.4%) |
| sit-and-win | 21/93 (22.6%) | **9/91 (9.9%)** | 23/90 (25.6%) | 12/92 (13.0%) |
| center opp rem /game | 0.47 | 0.30 | 0.40 | 0.48 |
| vacates /game | 1.33 | 1.35 | 1.35 | 1.26 |
| swaps /game (center) | 0.02 (0.01) | 0.16 (0.08) | 0.11 (0.07) | 0.07 (0.03) |
| converts /game (center) | 0 | **0.67 (0.01)** | 0 | **0.92 (0.01)** |
| CLAIM /game | 0 | 0.01 | 0.02 | 0 |
| single-braid center hits /game | 0 | 0 | 0.010 | 0 |
| fight_gain /game (mover/opp) | 0 | **0.70 (44/26)** | 0 | **0.82 (44/38)** |
| extra used CONVERT/SWAP/MOVE/ADD/CLAIM | 0/0/43/20/0 | **67/0/25/20/0** | 0/0/34/21/0 | **92/0/14/14/0** |
| leader after first cascade/fight | 41/41 (100%) | 64/66 (97.0%) | 35/36 (97.2%) | 66/69 (95.7%) |
| comebacks after that award | 0 | 2 (3.0%) | 1 | 3 (4.3%) |

Length buckets (100 games):

| turns | baseline | r1 | r2 | both |
|---|---|---|---|---|
| ≤20 | 3 | 6 | 3 | 4 |
| 21–40 | 41 | **57** | 40 | **57** |
| 41–60 | 27 | 17 | 24 | 17 |
| 61–80 | 9 | 5 | 6 | 4 |
| 81–120 | 8 | 6 | 14 | 4 |
| 121–199 | 11 | 6 | 11 | 12 |
| 200 | 1 | 3 | 2 | 2 |

Pairing split (P1 W-D-L, mean turns):

| | minimax vs mcts | mcts vs minimax |
|---|---|---|
| baseline | 25-0-25, 62.16 | 23-2-25, 54.66 |
| r1 | **39-3-8**, 56.68 | 17-0-33, 44.30 |
| r2 | 30-2-18, 67.56 | 19-0-31, 57.10 |
| both | **38-1-11**, 51.56 | 14-1-35, 54.58 |

Rule 1’s P1 bump is almost entirely **minimax as P1** (25→39 / 25→38).
MCTS as P1 gets worse (23→17 / 23→14). Conversion snowballs the seat
that already searches the extra.

### Extra-turn CONVERT at light (`any`)

Bots **use the flip**. r1: 67/112 extra actions are CONVERT (60%);
both: 92/120 (77%). That is higher uptake than the old position-exchange
SWAP (~30%). Remaining extras are still ADD/MOVE.

Where they flip (r1 67, both 92):

| target | r1 | both |
|---|---|---|
| outer / spawn | **49 (73%)** | **74 (80%)** |
| inner ring | 17 | 17 |
| origin | 1 | 1 |
| 3-ring / 2-ring / 1-ring | 33 / 33 / 1 | 47 / 40 / 5 |
| self-pop after steal | 0 | 0 |
| CONVERT that itself captures | 0 | 0 |

The typical extra is “turn over a distant 2- or 3-ring.” That is a
material pad, not a center-fight closer. 23/67 r1 converts (39/92
both) finish the game within 5 turns — usually lockout after the stolen
piece empties the opponent’s board.

## 4x (~20 games) r1 vs baseline

In-game `nodes:132000,iters:2600` both seats. n=20 is noisy. Conversion
**does** shorten at 4x, unlike the earlier position-exchange 4x.

| | 4x baseline | 4x r1 (convert) |
|---|---|---|
| P1 W-D-L | **14-1-5** | **10-0-10** |
| P1 win CI | 70.0% [48.1, 85.5] | 50.0% [29.9, 70.1] |
| mean / median | 78.35 / 58 | **64.10 / 36** |
| p90 | 144 | 140 |
| 200-turn draws | 1 | **0** |
| lockouts | 6 | 8 |
| never-left | 3/19 (15.8%) | 2/19 (10.5%) |
| sit-and-win | 2/19 (10.5%) | 2/19 (10.5%) |
| center opp rem /game | 0.65 | 0.30 |
| vacates /game | 2.25 | 1.90 |
| swaps /game | 0.15 | 0.10 |
| converts /game (center) | 0 | **0.50 (0.05)** |
| CLAIM | 0 | 0 |
| fight_gain /game | 0 | 0.55 (7 mover / 4 opp) |
| extra CONVERT/MOVE/ADD | 0/7/5 | **10/4/2** |
| leader after first award | 8/9 (88.9%) | 12/13 (92.3%) |
| comebacks | 1 | 1 (7.7%) |

Mean 78.35→**64.10**, median 58→**36**, the 200-turn draw is gone.
Lockouts 6→8. Seat balance flattens (14-1-5 → 10-0-10); n=20 is not
enough to trust that. CONVERT is 10/16 extra actions; 6 outer, 3
inner, 1 origin; 7× 3-ring, 3× 2-ring; 4 of 10 finish within 5 turns.

## Qualitative (cite log / pairing / game / turn)

JSONL reconstructs every action (`kind`, `from`/`to`, `captures`,
`fight_gain`, `turn_gain`, `played_as_extra`). CONVERT stores the stolen
piece in `piece` and the node in `to`.

### Snowball lockouts off a fight extra + outer CONVERT

The position-exchange teleports (outer↔outer deleting two tiles) are
gone. The new degenerate is **material theft**: steal a 2-ring or
3-ring on a spawn, opponent runs out of pieces, lockout.

- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 30 t15 / t20.**
  Two extra CONVERTs of outer 2-ring then 3-ring (`n5`, `n13`). Lockout
  the same turn as the second flip. Entered with more material, stole
  two distant pieces, game over.
- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 8 t28.** Extra
  CONVERT inner `n9` (3-ring) **on the lockout turn**.
- [`h2h_r1`](h2h_r1.jsonl.gz) **minimax vs mcts game 1 t30 / game 7
  t23 / game 16 t30 / game 26 t33.** P1 CONVERT of an outer 2- or
  3-ring, lockout same turn. This is the minimax-as-P1 39-3-8 cell.
- [`h2h_both`](h2h_both.jsonl.gz) **mcts vs minimax game 35 t36 / t38.**
  CONVERT outer `n23` (3-ring) then `n11` (2-ring); elimination at t38.
  Consecutive extras still chain.
- [`h2h_both`](h2h_both.jsonl.gz) **mcts vs minimax game 1 t15 / t24.**
  CONVERT outer `n11` twice (3-ring then 2-ring). Lockout t24.

### Center CONVERT is rare and terminal

- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 33 t33.** P2
  CONVERT **origin** (`n0`, 2-ring), lockout same turn. The one light
  r1 center flip.
- [`h2h_both`](h2h_both.jsonl.gz) **mcts vs minimax game 11 t35.** P2
  CONVERT origin (3-ring), control win same turn.
- [`h2h_deep4x_r1`](h2h_deep4x_r1.jsonl.gz) **mcts vs minimax game 9
  t49 / t58.** CONVERT origin (3-ring) then outer `n22` (3-ring).
  Lockout t58.

### CONVERT that does not close the game

- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 12 t82.** CONVERT
  outer `n14`; lockout only at t106. Long leftover walk after the
  extra.
- [`h2h_both`](h2h_both.jsonl.gz) **mcts vs minimax game 14 t96.**
  CONVERT inner `n3`; lockout t153.
- [`h2h_deep4x_r1`](h2h_deep4x_r1.jsonl.gz) **minimax vs mcts game 4
  t152.** CONVERT inner `n1` (3-ring) on the **elimination** turn of a
  152-turn game. Extra available in a walk that already failed to
  finish.

### Comebacks after the first extra (conversion freezes the lead)

Light r1 2/66 (3.0%), both 3/69 (4.3%). Position-exchange extras had
~12% comebacks. Conversion is stickier:

- r1 **mcts vs minimax game 28 t8** fight leader P2, P1 wins control t30
- r1 **minimax vs mcts game 32 t24** fight leader P2, P1 wins control t58
- both **mcts vs minimax game 21 t7** fight leader P2, P1 wins elimination t133
- 4x r1 **minimax vs mcts game 5 t10** fight leader P1, extra CONVERT t27
  outer `n23`, P2 wins control t181

Baseline (cascade extras only, no fight extras) was **41/41** — those
awards fire later, when the ratchet is already dropping. Fight extras
fire around t8–t25; conversion then usually holds.

### Center mismatch did not kill cheap walks

r2 never-left **rises** 24.7%→33.3%; mean length 58.41→62.33; vacates
flat (1.33→1.35). Unmatched 2-rings on the origin are **not**
overwhelmed by +1 (see
`center_mismatch_two_ring_unmatched_is_not_overwhelmed` in
`game.rs`). Exactplay’s cheap continuation is mostly 2-ring
inner↔center, so a 1-mismatch tax is a scoring nuisance, not a pop.

CLAIM of the origin still happens twice in r2 (same games as the first
reading):

- [`h2h_r2`](h2h_r2.jsonl.gz) **mcts vs minimax game 1 t105.** P1 CLAIM
  0 takes P2’s 2-ring off the origin. Lockout t113.
- [`h2h_r2`](h2h_r2.jsonl.gz) **mcts vs minimax game 38 t168.** P2 CLAIM
  0 of P1’s 1-ring (`cascade=opponent`); P1’s extra MOVE 19→0. Control
  P1 t175.

Combining r2 with conversion (`both`) does **not** beat r1 on length
and makes lockouts worse (40→46).

## Takeaways (not recommendations)

1. **Conversion is real and bots use it** (0.67–0.92 converts/game;
   60–77% of extra actions). Almost all flips are outer 2-/3-rings.
   Origin flips are ~1%. The piece stays; CONVERT itself never
   captured in these logs.
2. **Light length:** r1 shortens (58.41→50.49 mean, 43→34 median).
   `both` also shortens (53.07) but less, with more lockouts. Rule 2
   alone **lengthens**. 200-turn draws do not fall (1→3 / 2 / 2).
3. **Lockouts explode** (24→40 / 46). Stealing a distant piece is an
   elimination engine. Sit-and-win drops (22.6%→9.9%).
4. **Material-leader extra** already existed via `cascade_turn_gain`
   (100% conversion on those later awards). Fight extras fire earlier;
   CONVERT then **holds** (~3% comebacks vs ~12% under
   position-exchange SWAP). P1 seat bias is minimax-as-P1.
5. **Center +1 mismatch** still does not stop inner↔center walks.
   Combining it with conversion is not the shortener; conversion alone
   is.
6. **4x (n=20):** mean **does** drop (78.35→64.10), median 58→36, no
   200-turn draw. CONVERT still mostly outer. One 152-turn game flipped
   on the last ply.

## Open design questions

1. **CONVERT vs SWAP-only extra.** Bots CONVERT 60–77% of extras and
   still ADD/MOVE the rest. Should the extra turn be CONVERT-only?
2. **Who gets CONVERT.** All extras (including `cascade_turn_gain`) vs
   fight-gain extras only. Most CONVERT here is on fight extras or
   same-action fight+cascade, but cascade extras also unlock it.
3. **`any` vs `not-center` vs `inner`.** `any` is 73–80% outer/spawn
   theft. That is a material snowball, not a center-fight closer.
   `inner` would keep the extra on the control ring; `not-center`
   would still allow outer theft. Cheap variants are wired, not
   benched.
4. **Local-claim after CONVERT.** Stolen unmatched 1-rings can
   self-pop (“piece stays where it is” vs immediate pop). Observed
   self_pop=0 because bots steal 2-/3-rings. Fight-gain can fire if a
   converted origin occupant then pops (pre-action id among
   removals); not observed.
5. **Stacking.** Same-action fight+cascade banks one extra. Consecutive
   actions can chain extras (both mcts vs minimax game 35 t36–t38).
6. **Center mismatch magnitude.** +1 never pops a 2-ring. Options: +
   weight, treat unmatched origin as always overwhelmed, or only tax
   1-rings (already self-pop on empty origin). Combining with CONVERT
   made lockouts worse, not shorter.
7. **“Matched.”** HUD channel match (including 0-score braid-only-exact
   normals) vs scored match.
8. **Fight trigger.** Pre-action occupant captured, not first-entry
   snapshot, not vacate. Vacates still ~1.3/game. CONVERT of the
   origin without a pop does not re-award.
9. **Capture hold.** Spawn-token count, not `control_hold` turns.
10. **CLAIM / single-braid center hits** stay ~0. Conversion did not
    make CLAIM a bot plan.
11. **Seat / pairing.** Rule 1’s P1 bump is minimax-as-P1 lockout
    snowball. Worth a same-search pairing cell before treating it as a
    first-player effect.
12. **Position-exchange SWAP** remains `--extra-turn-action swap`. It
    is not re-benched; the earlier 4x `h2h_deep4x_both` in this folder
    is that old reading.
