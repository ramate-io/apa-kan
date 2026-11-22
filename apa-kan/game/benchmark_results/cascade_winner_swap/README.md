# Next-move ratchet reward (cascade / center fight)

Stacked on PR #85 (`exactplay`) / PR #83 (live playtest `RuleSet`).
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
in-game overlay `--p1-budget` / `--p2-budget`), `--jobs 4`,
`--no-depth-probe`, `--zags on`, `--zag-mismatch sum`, `--endgame-rule
control-cascade`, `--cascade-turn-gain on`, `--capture-mode
local-claim`, `--no-same-piece-twice off`, `--pick search`, `--pairings
minimax:mcts,mcts:minimax`, `--same-deck on`, `--search-profile
exactplay`.

**Seeds.** The earlier 100-game light cell used `--seed 1 --games 50`
(pairing 0 seeds 1–50, pairing 1 seeds 10001–10050). The 300-game cell
is a **new independent draw**, not an extension: `--seed 300 --games
150` (pairing 0 seeds 300–449, pairing 1 seeds 10300–10449). No overlap.

| Log | Flags | Games | Seed | Budget |
|---|---|---|---|---|
| [`h2h_n300_baseline`](h2h_n300_baseline.txt) | live (flag off) | **300** | 300 | light |
| [`h2h_n300_r1`](h2h_n300_r1.txt) | `--cascade-winner-swap on` | **300** | 300 | light |
| [`h2h_baseline`](h2h_baseline.txt) | live | 100 | 1 | light |
| [`h2h_r1`](h2h_r1.txt) | `--cascade-winner-swap on` | 100 | 1 | light |
| [`h2h_deep4x_baseline`](h2h_deep4x_baseline.txt) | live | 20 | 1 | 4x |
| [`h2h_deep4x_r1`](h2h_deep4x_r1.txt) | `--cascade-winner-swap on` | 20 | 1 | 4x |

Light baseline is **not** a byte-level replay of PR #85 (zobrist keys
grew).

## Light 300-game exactplay self-play (`--seed 300`)

Quoted from the `*.txt` logs plus [`survey.py`](survey.py) on the JSONL.
Wilson 95% CI on P1 wins / 300. Headline cell.

| | baseline | r1 (next-move ratchet) |
|---|---|---|
| P1 W-D-L | **137-5-158** | **139-3-158** |
| P1 win CI | 45.7% [40.1, 51.3] | 46.3% [40.8, 52.0] |
| mean / median turns | 59.19 / 42 | **56.14 / 40** |
| p90 | 133 | 123 |
| 200-turn draws | 5 | 3 |
| lockouts | 85 | **115** |
| lockouts right after flip/swap | 0/85 | **46/115 (40.0%)** |
| control | 192 | 165 |
| elim | 4 | 13 |
| never-left | 64/270 (23.7%) | **46/277 (16.6%)** |
| sit-and-win | 54/270 (20.0%) | **38/277 (13.7%)** |
| center opp rem /game | 0.48 | 0.34 |
| vacates /game | 1.29 | 1.54 |
| swaps /game (center) | 0.26 (0.10) | 0.46 (0.15) |
| converts /game (center) | 0 | **0.47 (0.01)** |
| CLAIM /game | 0.01 | 0.01 |
| cascade extras (`turn_gain`) /game | 0.69 (167/41) | **0** (replaced) |
| ratchet /game (mover/opp) | 0 | **1.51 (359/95)** |
| games with 1+ / 2+ ratchets | 0 / 0 | **220 / 158** |
| reward SWAP/CONVERT/MOVE/ADD | 0 | **71/140/111/132** |
| first-reward SWAP/MOVE/ADD | — | **60/66/94** |
| later-reward CONVERT/SWAP/MOVE/ADD | — | **140/11/45/38** |
| leader after first award | 134/140 (95.7%) cascade extras | 189/220 (85.9%) ratchets |
| comebacks after that award | **6/140 (4.3%)** | **30/220 (13.6%)** |

Length buckets (300 games):

| turns | baseline | r1 |
|---|---|---|
| ≤20 | 5 | 8 |
| 21–40 | 141 | 145 |
| 41–60 | 62 | 60 |
| 61–80 | 28 | 26 |
| 81–120 | 22 | 28 |
| 121–199 | 37 | 30 |
| 200 | 5 | 3 |

### Pairing split (engine, not seat)

Overall P1 is **flat** (137-5-158 vs 139-3-158). The n=100 47→58 bump
does not replicate. What does replicate is a large **engine** gap that
already exists on live rules:

| | minimax P1 vs mcts (n=150) | mcts P1 vs minimax (n=150) |
|---|---|---|
| baseline P1 W-D-L | **86-3-61** 57.3% [49.3, 65.0] | **51-2-97** 34.0% [26.9, 41.9] |
| r1 P1 W-D-L | **93-2-55** 62.0% [54.0, 69.4] | **46-1-103** 30.7% [23.8, 38.5] |
| mean / median | 60.49 / 45.5 → 59.77 / 43 | 57.89 / 38 → **52.52 / 37.5** |
| first-ratchet comebacks | 4/70 → **25/110 (22.7%)** | 2/70 → 5/110 (4.5%) |

Minimax-as-P1 ticks up 86→93; MCTS-as-P1 ticks down 51→46. CIs overlap
their baselines. The 22.7% comeback rate when minimax is P1 is mostly
minimax recovering after a P2-won first ratchet (example below), not a
first-player effect.

Ratchet mix (454 awards): **cascade 281, fight 148, both 25**. First
rewards still often skip SWAP (94 ADD + 66 MOVE vs 60 SWAP). Second+
**do** flip (140/234 = 60% CONVERT). CONVERT targets: outer 102, inner
35, origin 3; piece rings 3/2/1 = 75/59/6; **self_pop=0**, including
all six 1-ring flips. SWAP initiators: 68/71 multi-ring; 48/71 steal an
opponent piece; 20 involve the origin. CLAIM never used as a reward.

Lockouts after a reward CONVERT or SWAP: **42 CONVERT + 4 SWAP** of 115
(the other 69 lockouts end on MOVE/ADD/etc.).

## Light 100-game exactplay self-play (`--seed 1`)

Earlier cell. Wilson 95% CI on P1 wins / 100. **Do not pool with seed
300** (different deals). The 58% P1 figure is a small-n + minimax-P1
artifact; the 300-game cell is the one to quote for seat balance.

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
| cascade extras (`turn_gain`) /game | 0.64 (52/12) | **0** |
| ratchet /game (mover/opp) | 0 | **1.49 (113/36)** |
| games with 1+ / 2+ ratchets | 0 / 0 | **71 / 48** |
| reward SWAP/CONVERT/MOVE/ADD | 0 | **25/47/34/43** |
| comebacks after first award | **0/42** | **10/71 (14.1%)** |

Pairing split (P1 W-D-L, mean turns):

| | minimax vs mcts | mcts vs minimax |
|---|---|---|
| baseline | 25-0-25, 61.42 | 22-2-26, 56.62 |
| r1 | **40-0-10**, 49.40 | 18-0-32, 56.20 |

## 4x (~20 games, `--seed 1`)

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
| converts /game (center) | 0 | **0.85 (0.10)** |
| ratchet /game (mover/opp) | 0 | **1.80 (28/8)** |
| games with 1+ / 2+ | 0 / 0 | **12 / 10** |
| reward SWAP/CONVERT/MOVE/ADD | 0 | **5/17/5/9** |
| leader after first award | 8/9 (88.9%) | **12/12 (100%)** |
| comebacks | 1/9 (11.1%) | **0/12** |

## Qualitative (cite log / pairing / game / turn)

JSONL reconstructs every action (`kind`, `from`/`to`, `fight_gain`,
`ratchet_kind`, `ratchet_tier`, `played_as_ratchet`). CONVERT stores
the stolen piece in `piece` and the node in `to`.

### n=300, seed 300 — what is new vs the 100-game cell

- **Seat balance is noise.** Overall P1 45.7%→46.3%. The 100-game 58%
  P1 bump does not hold. Minimax-as-P1 is already 57% on live rules
  (86-3-61); r1 only adds a few wins (93-2-55). MCTS-as-P1 is already
  34% (51-2-97) and gets slightly worse (46-1-103).
- **Sit-and-win / never-left actually drop** (20.0%→13.7%, 23.7%→16.6%).
  The 100-game cell missed this (barely moved). More lockouts (85→115)
  and more vacates (1.29→1.54) — the origin is less of a camp.
- **40% of lockouts now end on a reward CONVERT or SWAP** (46/115 =
  42 CONVERT + 4 SWAP), up from 9/35 (26%) at n=100. Early example:
  [`h2h_n300_r1`](h2h_n300_r1.jsonl.gz) **mcts vs minimax game 1 t18.**
  Later CONVERT `n20` (2-ring) **on the lockout ply** of an 18-turn
  game.
- **1-ring CONVERT shows up** (6/140) with still **self_pop=0**.
  [`h2h_n300_r1`](h2h_n300_r1.jsonl.gz) **mcts vs minimax game 51 t34 /
  t37.** Two 1-ring CONVERTs (`n2`, `n8`); lockout t37. Chained 2+
  (three CONVERTs).
- **Origin CONVERT still rare** (3/140) and not the lockout ply:
  **mcts vs minimax game 13 t52** CONVERT origin 3-ring, control t52;
  **game 40 t89** origin 3-ring in a **comeback** (fight leader P2 at
  t11, P1 wins control t123); **game 56 t32** origin 2-ring, control
  t32.
- **First-tier SWAP lockouts exist but are rare** (2/115): **minimax vs
  mcts game 81 t59** SWAP `12<->0` (origin) P2 lockout; **game 110 t28**
  SWAP `18<->11` P1 lockout.
- **Later-tier SWAP** is 11/234 later rewards (still CONVERT-dominated).
- **Comebacks 30/220 (13.6%)** vs cascade-extra **6/140 (4.3%)** on the
  same deals. Almost all extra comebacks are **minimax as P1** (25/110
  vs 5/110 when MCTS is P1). Example: **minimax vs mcts game 4 t7**
  fight leader P2, P1 control t40. **mcts vs minimax game 40 t11** fight
  leader P2, P1 origin-CONVERT then control t123. No comeback in this
  cell ends on a CONVERT lockout.
- **First reward still often ADD/MOVE**, not SWAP (94+66 vs 60).

### n=100, seed 1 (kept for citation)

- [`h2h_r1`](h2h_r1.jsonl.gz) **minimax vs mcts game 13 t20.** P1
  first-tier SWAP, lockout same turn.
- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 18 t41 / t46.**
  Two later CONVERTs of outer 2-rings; lockout t46.
- [`h2h_r1`](h2h_r1.jsonl.gz) **mcts vs minimax game 2 t18** fight
  leader P2, P1 CONVERT-lockout t116.

4x r1: [`h2h_deep4x_r1`](h2h_deep4x_r1.jsonl.gz) **mcts vs minimax game
3 t28–t35.** Cascade leader P2, SWAP origin then CONVERT origin,
lockout t35. Comebacks **0/12**.

## Takeaways (not recommendations)

1. **The next-move rule is real.** n=300: 1.51 ratchets/game, 220/300
   games award at least one, 158/300 reach the flip tier. Live cascade
   extras go to 0 while the flag is on.
2. **Overall P1 is unchanged** at n=300 (45.7% [40.1, 51.3] → 46.3%
   [40.8, 52.0]). The n=100 47→58 bump was minimax-as-P1 on 50 deals.
   Minimax is already a favorite as P1 on live rules (~57%); r1 does
   not create a first-player effect on top of that.
3. **Length:** n=300 mean 59.19→56.14, median 42→40 (milder than the
   n=100 59→53 / 43→37). 200-turn draws 5→3. 4x still shortens more
   (69→54 / 58→40) at n=20.
4. **Lockouts 85→115.** 46 of those 115 (40%) end on a reward CONVERT
   or SWAP (42 flip, 4 swap). Sit-and-win and never-left **drop**
   (20.0%→13.7%, 23.7%→16.6%) — the 100-game cell did not have the n
   to see this.
5. **First reward often ignores SWAP** (ADD/MOVE). Second+ **does**
   CONVERT (60%), still mostly outer 2-/3-rings. 6/140 are 1-rings;
   self_pop=0 on all 140.
6. **Comebacks after first ratchet 30/220 (13.6%)** vs 6/140 (4.3%)
   cascade extras on the same seed-300 deals (and 10/71 at n=100).
   Concentrated in minimax-as-P1 (25/110). Opponent-won ratchets are
   not a free extra go.

## Open design questions

1. **Should ADD/CLAIM stay legal on reward moves?** Natural reading:
   yes. n=300 bots ADD 94/220 first rewards (CLAIM 0). CONVERT-only
   2nd+ would be a cheap follow-up if Liam wants the flip forced.
2. **Do 3+ ratchets keep the flip tier?** Implemented as yes. Not
   configurable. 234 later rewards in 158 games implies ~76 third+
   events, all flip-capable.
3. **Flip scope `any` vs inner.** `any` is 102/140 outer. `--extra-convert-mode
   inner` is wired, not benched.
4. **Do cascade drops count?** Default yes (`--ratchet-cascade on`).
   281/454 n=300 awards are cascade-only; 148 are center fights. `off`
   restores live cascade extras and fight-only rewards.
5. **Pre-action vs post-capture leader** on cascade drops. New rule
   uses entering material; live extras used post-capture. The mover-ADD
   drop fixture awards the **opponent** under this flag.
6. **Tied material and tokens:** no award, count does not increment.
7. **Local-claim after CONVERT.** Stolen unmatched 1-rings can
   self-pop. Observed self_pop=0 even on the six 1-ring flips.
8. **Center +1 mismatch** is still in the binary, default off, not in
   this cell.
9. **Same-search pairing.** The n=300 split shows the P1 lean is
   engine-driven on **both** cells (minimax ~57–62% as P1, MCTS ~31–34%
   as P1). Overall P1 is ~46% either way. Same-engine pairing would
   still be the clean seat-balance check.
10. **4x n=20** cannot decide seat balance or the 0/12 comeback hold.
    Light n=300 already answers seat balance (flat) and comebacks
    (~14%, n=220).
