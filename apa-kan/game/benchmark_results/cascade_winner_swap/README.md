# Cascade-winner extra swap + center mismatch

Stacked on PR #85 (`exactplay` bots) / PR #83 (live playtest `RuleSet`).
Two **off-by-default** rule toggles. Live defaults are unchanged:
braid-only exact, single braids, additive channels, double-counted
center↔inner, ring weights 4:3:1, color drift 160, same-deck 27-tile
start, adjacent normal-only single SWAP with no swap-back,
`cascade_turn_gain` already on.

Liam wants cascades/center fights to terminate faster and give the
player who **entered with more material** a stronger advantage, without
turn-count tracking over the board.

Reproduce with [`run_h2h.sh`](run_h2h.sh). Survey:
[`survey.py`](survey.py) (reuses [`../exactplay/survey.py`](../exactplay/survey.py);
first center *take* still counts only if the landing piece is there
after captures). Quote numbers from the committed `*.txt` / JSONL in
this directory.

## What the flags do

| Flag | Default | Effect |
|---|---|---|
| `--cascade-winner-swap on\|off` | off | Extra turn when the **pre-action** center occupant is among this action’s removals. Leader is pre-action `total_material`, tie-break spawn-token count (“capture hold”). Tied material **and** tokens: no award. |
| `--extra-swap-mode any\|own\|not-center` | `any` | Partner filter for extra-turn SWAP. Only consulted while `cascade_winner_swap` is on **and** the side is playing an extra turn. |
| `--center-mismatch on\|off` | off | Unmatched center occupant scores a synthetic **+1 mismatch**. |

Missing JSONL fields replay as off / `any`.

### SWAP interpretation

Existing `Action::Swap` is a **position exchange** of two occupied
tiles, not a conversion of an opponent piece onto your side. Extra-turn
SWAP is the same mechanic with a looser filter:

- initiator: **any** of the mover’s pieces (not live `NormalOnly` 1-ring)
- partner: any occupied tile (`any`); own pieces only (`own`); any except neither node is the origin (`not-center`)
- not necessarily adjacent
- vacant nodes are never partners
- one-move no-echo still applies
- does not spend a spawn token

**Open question (alternative reading):** “gets an extra turn, which they
may use to perform a swap” could mean converting an opponent piece to
your side, or a SWAP-only extra (MOVE/ADD illegal). This package keeps
the existing SWAP verb and still allows ADD/MOVE/CLAIM on the extra
turn. Cheap variants (`own`, `not-center`) are wired but **not**
benched; every cell here uses `any`.

Extra-turn SWAP is legal on **every** extra turn while the flag is on,
including extras from live `cascade_turn_gain`. Fight-gain does **not**
stack a second extra if cascade-drop already banked one on the same
action (`# fight_gain` still counts the fight).

Fight-gain does **not** fire on empty-center unmatched self-pops (the
cheap inner↔center walk). The captured tile must have been sitting on
the origin *before* the action.

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
| [`h2h_r1`](h2h_r1.txt) | `--cascade-winner-swap on` | 100 | light |
| [`h2h_r2`](h2h_r2.txt) | `--center-mismatch on` | 100 | light |
| [`h2h_both`](h2h_both.txt) | both on | 100 | light |
| [`h2h_deep4x_baseline`](h2h_deep4x_baseline.txt) | both off | 20 | 4x |
| [`h2h_deep4x_both`](h2h_deep4x_both.txt) | both on | 20 | 4x |

Light baseline is a **byte-level replay** of PR #85
[`exactplay/h2h_self`](../exactplay/h2h_self.txt): P1 **48-2-50**, mean
**58.41**, lockout 24, control 71, turn_limit 1.

**Best combo for the 4x check:** `both`. It is the only light cell
that shortened mean **and** median length, killed the 200-turn draw,
and actually used extra SWAP. Rule 1 alone lengthened games and raised
lockouts; rule 2 alone lengthened games and **raised** never-left.

## Light 100-game exactplay self-play

Quoted from the `*.txt` logs plus [`survey.py`](survey.py) on the JSONL
(median / never-left / sit-and-win / vacates / extra-turn use /
leader-after-first-award).

| | baseline | r1 (fight extra) | r2 (center +1) | both |
|---|---|---|---|---|
| P1 W-D-L | **48-2-50** | **58-2-40** | **49-2-49** | **56-0-44** |
| mean / median turns | 58.41 / 43 | 60.86 / 40.5 | 62.33 / 46 | **56.62 / 36** |
| p90 | 128 | 139 | 135 | 123 |
| 200-turn draws | 1 | 2 | 2 | **0** |
| lockouts | 24 | **30** | 22 | 25 |
| control | 71 | 61 | 71 | 68 |
| never-left | 23/93 (24.7%) | 21/94 (22.3%) | **30/90 (33.3%)** | 24/89 (27.0%) |
| sit-and-win | 21/93 (22.6%) | 19/94 (20.2%) | 23/90 (25.6%) | 20/89 (22.5%) |
| center opp rem /game | 0.47 | 0.29 | 0.40 | 0.34 |
| vacates /game | 1.33 | 1.31 | 1.35 | 1.47 |
| swaps /game (center) | 0.02 (0.01) | **0.35 (0.07)** | 0.11 (0.07) | **0.66 (0.05)** |
| CLAIM /game | 0 | 0.02 | 0.02 | 0.01 |
| single-braid center hits /game | 0 | 0.010 | 0.010 | 0 |
| fight_gain /game (mover/opp) | 0 | **0.52 (33/19)** | 0 | **0.58 (37/21)** |
| extra used SWAP/MOVE/ADD/CLAIM | 0/43/20/0 | **29/38/30/0** | 0/34/21/0 | **33/42/39/0** |
| leader after first cascade/fight | 41/41 (100%) | 51/58 (87.9%) | 35/36 (97.2%) | 47/54 (87.0%) |
| comebacks after that award | 0 | 7 (12.1%) | 1 | 7 (13.0%) |

Length buckets (100 games):

| turns | baseline | r1 | r2 | both |
|---|---|---|---|---|
| ≤20 | 3 | 2 | 3 | 2 |
| 21–40 | 41 | 48 | 40 | **53** |
| 41–60 | 27 | 22 | 24 | 15 |
| 61–80 | 9 | 6 | 6 | 11 |
| 81–120 | 8 | 6 | 14 | 8 |
| 121–199 | 11 | 14 | 11 | 11 |
| 200 | 1 | 2 | 2 | 0 |

Pairing split (P1 W-D-L, mean turns):

| | minimax vs mcts | mcts vs minimax |
|---|---|---|
| baseline | 25-0-25, 62.16 | 23-2-25, 54.66 |
| r1 | **36-1-13**, 56.3 | 22-1-27, 65.4 |
| r2 | 30-2-18, 67.6 | 19-0-31, 57.1 |
| both | **37-0-13**, 51.0 | 19-0-31, 62.2 |

Rule 1’s P1 bump is almost entirely **minimax as P1** (25→36 / 25→37).
MCTS as P1 does not pick it up.

### Extra-turn SWAP at light (`any`)

r1: 29 extra SWAPs. 26/29 multi-ring initiator (illegal under live
`NormalOnly`). 22 opponent partners, 7 own, 7 involve the origin, **7
are fully off the control ring** (true any-piece teleports). Source of
the extra (previous award on the JSONL): 14 `fight+cascade` same
action, 10 cascade-mover, 5 cascade-opponent. Bots still spend extra
turns on MOVE/ADD more often than SWAP (38+30 vs 29).

both: 33 extra SWAPs, **0 involving the origin**, 8 outer, 26 opponent,
33/33 multi-ring initiator. Center mismatch appears to make a center
extra SWAP unattractive; the teleport moves to inner/outer instead.

## 4x (~20 games) best combo vs baseline

In-game `nodes:132000,iters:2600` both seats. n=20 is noisy (PR #85’s
4x self-play was n=10, mean **101.00**). This 4x baseline came in
shorter than that historical cell.

| | 4x baseline | 4x both |
|---|---|---|
| P1 W-D-L | **14-1-5** | **9-0-11** |
| mean / median | 78.35 / 58 | 79.00 / 53 |
| p90 | 144 | 169 |
| 200-turn draws | 1 | **0** |
| lockouts | 6 | 7 |
| never-left | 3/19 (15.8%) | 2/19 (10.5%) |
| sit-and-win | 2/19 (10.5%) | 2/19 (10.5%) |
| center opp rem /game | 0.65 | 0.75 |
| vacates /game | 2.25 | **1.65** |
| swaps /game | 0.15 | 0.50 |
| CLAIM | 0 | 0 |
| fight_gain /game | 0 | 0.80 (2 mover / 14 opp) |
| extra SWAP/MOVE/ADD | 0/7/5 | **10/5/8** |
| leader after first award | 8/9 (88.9%) | 10/13 (76.9%) |

4x does **not** shorten mean length (78.35 → 79.00). Median dips 58→53
and the 200-turn draw disappears, but p90 gets worse (a 198-turn
control win with two late extra SWAPs). Vacates drop (2.25→1.65).
Seat balance flips relative to this 4x baseline; n=20 is not enough to
trust that. Extra SWAP is used (10/23 extra actions) and still never
touches the origin.

## Qualitative (cite log / pairing / game / turn)

JSONL reconstructs every action (`kind`, `from`/`to`, `captures`,
`fight_gain`, `turn_gain`, `played_as_extra`, `partner_owner`).

### Snowball lockouts off a fight extra + any-piece SWAP

- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 9 t11–t13.** P1
  MOVE 8→6 pops the origin (`fight=opponent`). P2’s extra SWAP 9↔17
  (inner↔outer, 2-ring vs 2-ring) removes P1 at 9 **and** 11. Lockout
  at t19. Classic “entered with more material, extra SWAP as a double
  capture, game over in a few plies.”
- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 42 t16–t17.** P2
  MOVE 0→18 self-pops the origin (`fight=mover`) then extra SWAP
  **22↔5** — both nodes off the control ring — capturing P1 at 20 and
  22. Elimination at t37 after a second extra SWAP 21↔11. This is the
  permissive `any` reading at its most degenerate: a 3-ring on an outer
  spawn teleports onto another outer and deletes two of P1’s tiles.
- [`h2h_both`](h2h_both.jsonl.gz) **mcts vs minimax game 42 t22–t23.**
  Same shape: origin self-pop, extra SWAP 9↔20 capturing two P1 tiles,
  lockout t32.
- [`h2h_both`](h2h_both.jsonl.gz) **minimax vs mcts game 46 t32–t34.**
  Cascade extra SWAP 9↔11 pops the origin **and** P2 at 9
  (`fight=mover` on the extra SWAP itself), then a **second** extra
  MOVE 14→19 takes P2’s last spawn. Lockout t34. Extra SWAP as a
  center-clearing capture, then a chained extra.

### Consecutive extra SWAPs

- [`h2h_both`](h2h_both.jsonl.gz) **minimax vs mcts game 2 t37–t39.**
  Cascade-mover ADD, extra SWAP 1↔11 (own partner, self-capture at 1)
  which **itself** cascade-awards, then a second extra SWAP 5↔18
  (opponent 3-ring). Control win t41. Two extra SWAPs in a row because
  extras can re-award; we only refuse stacking on the *same* action.

### Extra SWAP that does not close the game

- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 1 t156–t163.**
  167-turn control win. Cascade-opponent at t156, extra SWAP **0↔22**
  (center, capturing origin P2 and inner P1), then another extra SWAP
  20↔12 (own 3-ring on inner). Long center walk **after** the extra
  SWAP. Fight extras do not kill 100+ turn games by themselves.
- [`h2h_r1`](h2h_r1.jsonl.gz) **minimax vs mcts game 20 t35.** Extra
  SWAP 0↔23 (center, no captures) then P2 immediately fights back
  (`fight=opponent` t36) and **eliminates P1** at t57. Leader after
  first fight was P1; comeback.
- [`h2h_both`](h2h_both.jsonl.gz) **mcts vs minimax game 2 t10–t11.**
  Fight-mover extra SWAP 11↔6 (no captures). P2 still loses t117.
  Comeback after the extra.

### Comebacks after the first fight extra (rule 1 does not freeze the lead)

Light r1 7/58 (12.1%), both 7/54 (13.0%). Long ones:

- r1 **mcts vs minimax game 11 t17** fight leader P2, P1 wins control t177
- r1 **mcts vs minimax game 14 t23** fight leader P2, P1 wins control t160
- both **minimax vs mcts game 18 t8** fight leader P1, P2 wins control t159
- 4x both **minimax vs mcts game 6 t26** fight leader P1, extra SWAP t28
  20↔21, P2 wins control t164

Baseline (cascade extras only, no fight extras) was **41/41** — those
awards fire later, when the ratchet is already dropping. Fight extras
fire around t8–t25, so the lead is less baked in.

### Center mismatch did not kill cheap walks

r2 never-left **rises** 24.7%→33.3%; mean length 58.41→62.33; vacates
flat (1.33→1.35). Unmatched 2-rings on the origin are **not**
overwhelmed by +1 (see
`center_mismatch_two_ring_unmatched_is_not_overwhelmed` in
`game.rs`). Exactplay’s cheap continuation is mostly 2-ring
inner↔center, so a 1-mismatch tax is a scoring nuisance, not a pop.

One new behavior: CLAIM of the origin actually happens.

- [`h2h_r2`](h2h_r2.jsonl.gz) **mcts vs minimax game 1 t105.** P1 CLAIM
  0 takes P2’s 2-ring off the origin. Lockout t113. (Live CLAIM was 0
  in the 100-game exactplay self-play.)
- [`h2h_r2`](h2h_r2.jsonl.gz) **mcts vs minimax game 38 t168.** P2 CLAIM
  0 of P1’s 1-ring (`cascade=opponent`); P1’s extra MOVE 19→0. Control
  P1 t175. CLAIM as a late center-clear, not a snowball.

### 4x extra SWAP still used as a mid/late teleport, not a terminator

- [`h2h_deep4x_both`](h2h_deep4x_both.jsonl.gz) **minimax vs mcts game 4
  t194 / t197.** Two extra SWAPs at the end of a **198-turn** control
  win (8↔12 then 14↔7, both 2-ring, one outer). Degenerate: the extra
  is available in a walk that already failed to finish.
- [`h2h_deep4x_both`](h2h_deep4x_both.jsonl.gz) **mcts vs minimax game 1
  t26.** Extra SWAP 2↔16, game still goes to t178.
- [`h2h_deep4x_both`](h2h_deep4x_both.jsonl.gz) **mcts vs minimax game 5
  t31.** Extra SWAP 10↔3, control win **same turn**. This is the 4x
  case where the extra actually finishes.

CLAIM stays 0 at 4x. Single-braid center hits stay ~0.

## Takeaways (not recommendations)

1. **Permissive extra SWAP is real and bots use it** (~0.3–0.7
   swaps/game vs 0.02 live). Mostly multi-ring vs opponent, often off
   the origin, often as a double capture. ~30% of extra turns are SWAP;
   the rest are still ADD/MOVE.
2. **Light length:** only `both` shortens (58.41→56.62 mean, 43→36
   median, 0 turn-limit). Rule 1 or 2 alone **lengthen**. Lockouts do
   not fall (24→30 / 22 / 25).
3. **Material-leader extra** already existed via `cascade_turn_gain`
   (100% conversion on those later awards). Fight extras fire earlier
   and **re-open comebacks** (~12%). P1 seat bias appears when minimax
   sits P1.
4. **Center +1 mismatch** does not stop inner↔center walks. never-left
   got worse. Combining it with rule 1 is what trimmed the left tail
   (53 games in 21–40) and made extra SWAP avoid the origin.
5. **4x (n=20):** mean length does not drop. Extra SWAP still happens;
   a 198-turn game used two of them. Vacates down, lockouts not.

## Open design questions

1. **SWAP verb.** Position exchange (this PR) vs converting an opponent
   piece to your side. The latter would need new rules and is not a
   toggle on `Action::Swap`.
2. **Extra action set.** Should the extra turn be SWAP-only? Bots
   currently ADD/MOVE more than SWAP on extras.
3. **Who gets extra SWAP.** All extras (including `cascade_turn_gain`)
   vs fight-gain extras only. Most extra SWAPs here are on cascade
   extras or same-action fight+cascade.
4. **`any` vs `own` vs `not-center` vs adjacency.** `any` produces
   outer↔outer teleports that delete two tiles (r1 mcts vs minimax
   game 42 t17). `not-center` still allows that. An adjacent-only extra
   SWAP was not implemented (live SWAP is already adjacent).
5. **Stacking.** Same-action fight+cascade banks one extra. Consecutive
   actions can chain extras (both minimax vs mcts game 2 t38–t39).
6. **Center mismatch magnitude.** +1 never pops a 2-ring. Options: +
   weight, treat unmatched origin as always overwhelmed, or only tax
   1-rings (already self-pop on empty origin).
7. **“Matched.”** HUD channel match (including 0-score braid-only-exact
   normals) vs scored match. Scored-only would keep unmatched painted
   normals taxed on the origin.
8. **Fight trigger.** Pre-action occupant captured, not first-entry
   snapshot, not vacate. Vacates still ~1.3–1.5/game.
9. **Capture hold.** Spawn-token count, not `control_hold` turns.
10. **CLAIM / single-braid center hits** stay ~0. The extra turn did
    not make CLAIM a bot plan except two late r2 origin CLAIMs.
11. **Seat / pairing.** Rule 1’s P1 bump is minimax-as-P1. Worth a
    same-search pairing cell before treating it as a first-player
    rule.
12. **4x n.** 20 games cannot decide whether `both` helps under deeper
    search. The 198-turn extra-SWAP game is a warning, not a rate.
