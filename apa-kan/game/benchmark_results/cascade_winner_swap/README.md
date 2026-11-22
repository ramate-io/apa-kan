# Next-move ratchet reward (cascade / center fight)

Stacked on PR #85 (`exactplay` bots) / PR #83 (live playtest `RuleSet`).
`--cascade-winner-swap` is **off by default**. Live defaults are
unchanged: braid-only exact, single braids, additive channels,
double-counted center↔inner, ring weights 4:3:1, color drift 160,
same-deck 27-tile start, adjacent normal-only single SWAP with no
swap-back, `cascade_turn_gain` already on.

Liam’s simplified reading (this cell): there is **no extra turn per
se**. After a ratchet, the **next move** goes to the winner (higher
material entering; tie-break spawn tokens / “capture hold”). If that
player would move next anyway (the opponent triggered it), play
continues as normal. On the move immediately after the **first**
ratchet of the game the winner may MOVE or SWAP positions (loosened
filter). After the **second and every later** ratchet they may also
FLIP (`Action::Convert`: opponent piece stays, owner changes).

This cell is **baseline vs that rule**. Center +1 mismatch is not
benched here (`--center-mismatch` stays off). Older `h2h_r2` /
`h2h_both` / `h2h_deep4x_both` files in this folder are leftover from
the previous extra-turn CONVERT reading and were not regenerated.

Reproduce with [`run_h2h.sh`](run_h2h.sh). Survey:
[`survey.py`](survey.py) (reuses [`../exactplay/survey.py`](../exactplay/survey.py)).
Quote numbers from the committed `*.txt` / JSONL. P1 win-rate CI is
Wilson 95% on wins/n.

## What the flag does

| Flag | Default | Effect |
|---|---|---|
| `--cascade-winner-swap on\|off` | off | After a ratchet, seat the winner next. First reward: loosened SWAP. Second+: SWAP + CONVERT. |
| `--ratchet-cascade on\|off` | on | Count non-winning control-cascade drops as ratchets (and suppress live `cascade_turn_gain` extras for those drops). `off` keeps live cascade extras and only treats center fights as ratchets. |
| `--extra-convert-mode any\|not-center\|inner` | `any` | CONVERT target filter on 2nd+ rewards. Cheap variants wired, not benched. |
| `--extra-swap-mode any\|own\|not-center` | `any` | Partner filter for post-ratchet SWAP. |
| `--center-mismatch on\|off` | off | Unmatched center occupant +1 mismatch. Not in this bench. |

`--extra-turn-action` is **removed**. The old extra-turn CONVERT vs SWAP
switch does not apply: both verbs exist, gated by ratchet index.

### Natural readings (this implementation)

- **Ratchet** = pre-action center occupant captured **or** a
  non-winning control-cascade checkpoint drop. One applied action is at
  most one ratchet (a drop that also captures the origin occupant is
  `both`). Immediate unique-holder drop wins do not award a reward
  (game over).
- **Winner** = higher **pre-action** `total_material`, then spawn-token
  count. Tied both ways: no award, count does not increment. This
  differs from live `cascade_turn_gain` (post-capture strict material,
  no token tie-break).
- **No extra-turn bank.** Mover-won ratchets keep the mover to play
  (that is the only extra seating). Opponent-won ratchets leave the
  upcoming opponent turn in place and attach the bonus verbs to it.
- **Game-global count.** First ratchet of the game is SWAP-tier.
  Second and later keep the flip tier (3+ is not a third privilege).
- **ADD / CLAIM / live SWAP** stay legal on reward moves (“normal
  options”). CONVERT is extra on 2nd+ only.
- While the flag is on and `--ratchet-cascade on`, live cascade extras
  are **not** banked for drops this flag already counts.

Missing JSONL: `cascade_winner_swap` / `center_mismatch` replay as off;
`ratchet_cascade` as on; convert/swap modes as `any`. Older extra-turn
CONVERT logs on this branch cannot faithfully replay (seating changed).

HUD: `ratchet-1 swap-any` / `ratchet-2+ flip-any`. Click an opponent
piece on a 2nd+ reward issues CONVERT.

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
| [`h2h_baseline`](h2h_baseline.txt) | live (flag off) | 100 | light |
| [`h2h_r1`](h2h_r1.txt) | `--cascade-winner-swap on` | 100 | light |
| [`h2h_deep4x_baseline`](h2h_deep4x_baseline.txt) | live | 20 | 4x |
| [`h2h_deep4x_r1`](h2h_deep4x_r1.txt) | `--cascade-winner-swap on` | 20 | 4x |

Light baseline is **not** a byte-level replay of PR #85 (zobrist keys
grew). P1 **47-2-51**, mean **59.02**, lockout 23, control 72,
turn_limit 1 — close to the previous 48-2-50 / 58.41 cell.

## Light 100-game exactplay self-play

Quoted from the `*.txt` logs plus [`survey.py`](survey.py) on the JSONL.
Wilson 95% CI on P1 wins / 100.

| | baseline | r1 (next-move ratchet) |
|---|---|---|
| P1 W-D-L | **47-2-51** | **58-0-42** |
| P1 win CI | 47.0% [37.5, 56.7] | 58.0% [48.2, 67.2] |
| mean / median turns | 59.02 / 43 | **52.80 / 37** |
| p90 | 128 | 121 |
| 200-turn draws | 1 | **0** (one 200-turn **control** win) |
| lockouts | 23 | **35** |
| lockouts right after flip/swap | 0/23 | **9/35 (25.7%)** |
| control | 72 | 60 |
| never-left | 23/94 (24.5%) | 21/91 (23.1%) |
| sit-and-win | 21/94 (22.3%) | 19/91 (20.9%) |
| center opp rem /game | 0.49 | 0.25 |
| vacates /game | 1.35 | 1.36 |
| swaps /game (center) | 0.02 (0.01) | 0.27 (0.04) |
| converts /game (center) | 0 | **0.47 (0.03)** |
| CLAIM /game | 0 | 0.01 |
| cascade extras (`turn_gain`) /game | 0.64 (52/12) | **0** (replaced) |
| ratchet /game (mover/opp) | 0 | **1.49 (113/36)** |
| games with 1+ / 2+ ratchets | 0 / 0 | **71 / 48** |
| reward SWAP/CONVERT/MOVE/ADD | 0 | **25/47/34/43** |
| first-reward SWAP/MOVE/ADD | — | **20/22/29** |
| later-reward CONVERT/SWAP/MOVE/ADD | — | **47/5/12/14** |
| leader after first award | 42/42 (100%) cascade extras | 61/71 (85.9%) ratchets |
| comebacks after that award | **0/42** | **10/71 (14.1%)** |

Length buckets (100 games):

| turns | baseline | r1 |
|---|---|---|
| ≤20 | 3 | 3 |
| 21–40 | 41 | **54** |
| 41–60 | 26 | 20 |
| 61–80 | 9 | 6 |
| 81–120 | 9 | 6 |
| 121–199 | 11 | 10 |
| 200 | 1 | 1 |

Pairing split (P1 W-D-L, mean turns):

| | minimax vs mcts | mcts vs minimax |
|---|---|---|
| baseline | 25-0-25, 61.42 | 22-2-26, 56.62 |
| r1 | **40-0-10**, 49.40 | 18-0-32, 56.20 |

P1 bump is again **minimax as P1** (25→40). MCTS as P1 gets worse
(22→18).

Ratchet mix (149 awards): **cascade 92, fight 53, both 4**. First
rewards often skip the special SWAP (29 ADD + 22 MOVE vs 20 SWAP).
Second+ rewards **do** flip (47/78 = 60% CONVERT). CONVERT targets:
outer 26, inner 18, origin 3; rings 3/2/1 = 30/16/1; **self_pop=0**,
CONVERT itself never captured. SWAP initiators are all multi-ring;
20/25 steal an opponent piece.

## 4x (~20 games)

In-game `nodes:132000,iters:2600` both seats. n=20 is noisy.

| | 4x baseline | 4x r1 |
|---|---|---|
| P1 W-D-L | **15-0-5** | **13-0-7** |
| P1 win CI | 75.0% [53.1, 88.8] | 65.0% [43.3, 81.9] |
| mean / median | 69.45 / 58 | **54.15 / 40** |
| p90 | 139 | 124 |
| 200-turn draws | 0 | 0 |
| lockouts | 7 | 7 |
| lockouts right after flip/swap | 0/7 | **3/7** |
| never-left | 3/19 (15.8%) | 5/19 (26.3%) |
| sit-and-win | 2/19 (10.5%) | 3/19 (15.8%) |
| center opp rem /game | 0.70 | 0.35 |
| vacates /game | 2.25 | — |
| swaps /game (center) | 0.15 (0.10) | 0.30 (0.15) |
| converts /game (center) | 0 | **0.85 (0.10)** |
| cascade extras /game | 0.65 | **0** |
| ratchet /game (mover/opp) | 0 | **1.80 (28/8)** |
| games with 1+ / 2+ | 0 / 0 | **12 / 10** |
| reward SWAP/CONVERT/MOVE/ADD | 0 | **5/17/5/9** |
| leader after first award | 8/9 (88.9%) | **12/12 (100%)** |
| comebacks | 1/9 (11.1%) | **0/12** |

Mean 69.45→**54.15**, median 58→**40**. CONVERT is 17/36 rewards
(47%); 12 outer, 3 inner, 2 origin. 4x first-ratchet leader **held**
(0 comebacks, n=12) while light r1 had 10/71. n=20 cannot decide seat
balance (15-0-5 → 13-0-7).

## Qualitative (cite log / pairing / game / turn)

JSONL reconstructs every action (`kind`, `from`/`to`, `fight_gain`,
`ratchet_kind`, `ratchet_tier`, `played_as_ratchet`). CONVERT stores
the stolen piece in `piece` and the node in `to`.

### First-ratchet SWAP is a teleport, not a flip

- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 2 t19.** P2
  first-tier SWAP `2<->12` (2-ring vs P1). P1 later CONVERT `n13`
  (2-ring) **on the lockout ply** t116. Comeback: fight leader P2 at
  t18, P1 wins.
- [`h2h_r1`](h2h_r1.jsonl.gz) **minimax vs mcts game 13 t20.** P1
  first-tier SWAP, **lockout same turn**. One of 9 light lockouts
  whose last action is flip/swap.
- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 37 t84.** P1
  first-tier SWAP involving the origin (`6<->0`), control t86.

### Second+ CONVERT still steals outer 2-/3-rings

- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 18 t41 / t46.**
  Two later CONVERTs of outer 2-rings (`n16`, `n14`); lockout t46.
- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 21 t24 / t26 /
  t31.** Three later CONVERTs including **origin** (3-ring) then two
  outers; control t33. Chained 2+ rewards.
- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 22 t34.** P2
  CONVERT `n3` (2-ring) **on the lockout ply**.
- [`h2h_r1`](h2h_r1.jsonl.gz) **minimax vs mcts game 28 t25 / game 40
  t35.** P1 CONVERT lockout same turn (minimax-as-P1 40-0-10 cell).

### Comebacks after the first ratchet (n=71 light, n=12 at 4x)

Light **10/71 (14.1%)** vs baseline cascade-extra **0/42**. The new
rule is **less** sticky than extra-turn CONVERT (~3%) because
opponent-won ratchets are not an extra go — they only attach verbs to
a turn that was coming anyway.

- r1 **mcts vs minimax game 2 t18** fight leader P2, P1 CONVERT-lockout t116
- r1 **mcts vs minimax game 24 t13** fight leader P2, P1 control t61
- r1 **mcts vs minimax game 32 t13** fight leader P2, P1 lockout t63
- r1 **mcts vs minimax game 48 t7** fight leader P2, P1 control t121
- r1 **minimax vs mcts game 2 t47** cascade leader P2, P1 control t60
- r1 **minimax vs mcts game 14 t14** fight leader P1, P2 control t147

4x r1: **0/12** comebacks. Example hold:
[`h2h_deep4x_r1`](h2h_deep4x_r1.jsonl.gz) **mcts vs minimax game 3
t28–t35.** Cascade leader P2, SWAP origin then CONVERT origin, lockout
t35.

### ADD is used on reward moves

First-tier ADD 29 / MOVE 22 / SWAP 20. Later ADD 14. Bots often take
the “normal option” instead of the special verb, especially on the
first ratchet.

## Takeaways (not recommendations)

1. **The next-move rule is real.** 1.49 ratchets/game, 71/100 games
   award at least one, 48/100 reach the flip tier. Live cascade extras
   go to 0 while the flag is on.
2. **Length:** light 59.02→52.80 mean, 43→37 median. 4x 69.45→54.15 /
   58→40. 200-turn **draws** 1→0 (one 200-turn control win remains).
3. **Lockouts 23→35.** 9 of those 35 end on a reward CONVERT or SWAP.
   Sit-and-win and never-left barely move (22.3%→20.9%, 24.5%→23.1%).
4. **First reward often ignores SWAP** (ADD/MOVE). Second+ **does**
   CONVERT (60%), still mostly outer 2-/3-rings, self_pop=0.
5. **Comebacks after first ratchet 10/71 (14%)** vs 0/42 cascade extras
   on baseline and ~3% under the old extra-turn CONVERT. Opponent-won
   ratchets no longer grant a free extra go.
6. **P1 bump is minimax-as-P1** (25→40). MCTS as P1 loses more. 4x
   flattens (15-0-5 → 13-0-7); n=20.

## Open design questions

1. **Should ADD/CLAIM stay legal on reward moves?** Natural reading:
   yes (“normal options”). Bots ADD 29/71 first rewards. CONVERT-only
   2nd+ would be a cheap follow-up if Liam wants the flip forced.
2. **Do 3+ ratchets keep the flip tier?** Implemented as yes. Not
   configurable. 78 later rewards in 48 games implies ~30 third+
   events, all flip-capable.
3. **Flip scope `any` vs inner.** `any` is 26/47 outer. `--extra-convert-mode
   inner` is wired, not benched.
4. **Do cascade drops count?** Default yes (`--ratchet-cascade on`).
   92/149 light awards are cascade-only; 53 are center fights. `off`
   restores live cascade extras and fight-only rewards.
5. **Pre-action vs post-capture leader** on cascade drops. New rule
   uses entering material; live extras used post-capture. The mover-ADD
   drop fixture awards the **opponent** under this flag.
6. **Tied material and tokens:** no award, count does not increment.
7. **Local-claim after CONVERT.** Stolen unmatched 1-rings can
   self-pop. Observed self_pop=0.
8. **Center +1 mismatch** is still in the binary, default off, not in
   this cell.
9. **Same-search pairing** before treating the P1 bump as a
   first-player effect.
10. **4x n=20** cannot decide seat balance or the 0/12 comeback hold.
