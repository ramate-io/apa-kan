# 2-2-1-1 opening (`--opening-double`)

Stacked on PR #86 (next-move ratchet) / PR #85 (`exactplay`).
`--opening-double` / `--opening 2-2-1-1` is **off by default**. Live
defaults are unchanged.

The question is seat balance. On the existing minimax-vs-minimax
mirror, P1 is **42.0%** with the ratchet on (126-9-165) and **43.7%**
under live rules. This cell asks whether giving P1 two consecutive
placements, then P2 two, then singles, pulls that toward 50%.

Reproduce with [`run_h2h.sh`](run_h2h.sh). Survey:
[`survey.py`](survey.py) (reuses
[`../cascade_winner_swap/survey.py`](../cascade_winner_swap/survey.py)
and [`../exactplay/survey.py`](../exactplay/survey.py)). Quote numbers
from the committed `*.txt` / JSONL. P1 win-rate CI is Wilson 95% on
wins/n. Search and eval are unchanged; this is a rules-only experiment.

## What the flag does

| Flag | Default | Effect |
|---|---|---|
| `--opening-double on\|off` | off | 2-2-1-1: P1 two consecutive 1-action turns, then P2 two, then singles. |
| `--opening 2-2-1-1\|1-1-1-1` | `1-1-1-1` | Alias (`double`/`on` = on, `single`/`off` = off). |

Each opening action is still one `finish_turn` (not `actions_per_turn=2`).
Turn count stays comparable to 1-1-1-1, and ratchet / lockout apply
**per action**. HUD shows `open-2` while the side to move still owes
the leftover. JSONL missing `opening_double` replays as off; each
action carries `played_as_opening` when it was one of the pair.

### Interaction with ratchets / extras / lockout

Seating priority in `finish_turn`: **pending ratchet > extra-turn bank >
opening stay > pass**.

- **Mover-won ratchet** on an opening action **consumes** the leftover
  stay. The reward action *is* the second seating (no third go). That
  ply is `played_as_ratchet`, not `played_as_opening`.
- **Opponent-won ratchet** **forfeits** the unused leftover. P2 still
  owes its double after P1 is interrupted.
- **Pass / no follow-up action** also forfeits the leftover.
- Lockout is checked for the player **about to move** after seating, as
  usual.
- `--actions 2` would stack with this flag (not benched). In 300 games
  no first-action ratchet fired on the opening (P1 `played_as_opening`
  n=2 in 300/300).

Zobrist keys for stays / pending are **appended** at the end of
`Zobrist::new()`, so default-off hashes are unchanged.

## Package

Release `apa-kan-bench`, `--no-default-features`, `--budget
nodes:33000,iters:650`, `--jobs 4`, `--no-depth-probe`, `--zags on`,
`--zag-mismatch sum`, doubles allowed (`--no-same-piece-twice off`),
`--endgame-rule control-cascade`, `--cascade-turn-gain on`,
`--capture-mode local-claim`, `--pick search`, `--pairings
minimax:minimax,minimax:minimax`, `--same-deck on`, `--search-profile
exactplay`. Headline arm adds `--opening-double on --cascade-winner-swap
on --ratchet-cascade on --extra-convert-mode any`. The cheap extra arm
is the same package with the ratchet flags off (live extras).

**Seeds.** Same independent draw as
[`../cascade_winner_swap/mirror/`](../cascade_winner_swap/mirror/):
`--seed 300 --games 150` (pairing 0 seeds 300–449, pairing 1 seeds
10300–10449). JSONL reconstructs every action. (`--record` sample dumps
only a minimax-vs-mcts game 0, so there is no sample record file here.)

| Log | Flags | Games | Seed | Budget |
|---|---|---|---|---|
| [`h2h_n300_mm_r1`](h2h_n300_mm_r1.txt) | 2-2-1-1 + r1 | **300** | 300 | light |
| [`h2h_n300_mm_baseline`](h2h_n300_mm_baseline.txt) | 2-2-1-1, live (no ratchet) | **300** | 300 | light |

## Minimax vs minimax (`--seed 300`, same deals)

Compared to [`../cascade_winner_swap/mirror/`](../cascade_winner_swap/mirror/)
on the same 300 deals.

| | mm-mm baseline (1-1-1-1) | mm-mm r1 (1-1-1-1) | **2211 baseline** | **2211 r1** |
|---|---|---|---|---|
| P1 W-D-L | **131-15-154** | **126-9-165** | **150-14-136** | **152-6-142** |
| P1 win CI | 43.7% [38.2, 49.3] | 42.0% [36.6, 47.7] | **50.0% [44.4, 55.6]** | **50.7% [45.0, 56.3]** |
| mean / median turns | 57.90 / 47 | 52.74 / 43 | **54.22 / 43** | **49.36 / 41.5** |
| p90 | 97 | 84 | 83 | **70** |
| 200-turn draws | 10 | 7 | 12 | 6 |
| lockouts | 35 | 71 | 41 | 75 |
| lockouts right after a ratchet reward | 0 | **47/71 (66.2%)** CONVERT 37, SWAP 2, MOVE 5, ADD 3 | 0 | **48/75 (64.0%)** CONVERT 39, SWAP 3, MOVE 2, ADD 4 |
| lockouts after flip/swap | 0 | 39/71 (54.9%) | 0 | **42/75 (56.0%)** |
| lockouts after a live cascade extra | **11/35 (ADD 6, MOVE 5)** | 0 | **15/41 (ADD 8, MOVE 7)** | 0 |
| control | 232 | 208 | 226 | 201 |
| never-left | 51/287 (17.8%) | 41/280 (14.6%) | 66/290 (22.8%) | 46/288 (16.0%) |
| sit-and-win | 30/287 (10.5%) | 30/280 (10.7%) | **44/290 (15.2%)** | 37/288 (12.8%) |
| contested origin | 234/287 (81.5%) | 229/280 (81.8%) | 220/290 (75.9%) | 233/288 (80.9%) |
| cascade extras /game | 0.70 (172/37) | **0** | 0.67 (165/36) | **0** |
| ratchet /game (mover/opp) | 0 | **1.73 (348/172)** | 0 | **1.84 (376/176)** |
| games with 1+ / 2+ | 0 / 0 | **241 / 171** | 0 / 0 | **237 / 187** |
| reward SWAP/CONVERT/MOVE/ADD | 0 | **101/168/109/137** | 0 | **105/184/118/142** |
| first-reward SWAP/MOVE/ADD | — | **88/60/90** | — | **91/48/97** |
| later-reward CONVERT/SWAP/MOVE/ADD | — | **168/13/49/47** | — | **184/14/70/45** |
| CONVERT outer/inner/origin | — | **107/45/16** (self_pop=0) | — | **114/54/16** (self_pop=0) |
| leader after first award | 128/134 (95.5%) cascade extras | 192/241 (79.7%) ratchets | 127/134 (94.8%) cascade extras | 172/237 (72.6%) ratchets |
| comebacks after that award | **3/134 (2.2%)** | **44/241 (18.3%)** | **4/134 (3.0%)** | **63/237 (26.6%)** |

Length buckets (300 games):

| turns | mm-mm baseline | mm-mm r1 | 2211 baseline | 2211 r1 |
|---|---|---|---|---|
| ≤20 | 1 | 1 | 1 | 1 |
| 21–40 | 102 | 131 | 131 | **144** |
| 41–60 | 116 | 110 | 104 | 104 |
| 61–80 | 39 | 25 | 32 | 31 |
| 81–120 | 21 | 17 | 13 | 8 |
| 121–199 | 11 | 9 | 7 | 6 |
| 200 | 10 | 7 | 12 | 6 |

Seed-block split (same deals as the existing mirror):

| | seeds 300–449 (n=150) | seeds 10300–10449 (n=150) |
|---|---|---|
| mm-mm baseline | 61-9-80 40.7% [33.1, 48.7] | 70-6-74 46.7% [38.9, 54.6] |
| mm-mm r1 | 62-7-81 41.3% [33.8, 49.3] | 64-2-84 42.7% [35.0, 50.7] |
| **2211 baseline** | **71-8-71 47.3% [39.5, 55.3]** | **79-6-65 52.7% [44.7, 60.5]** |
| **2211 r1** | **65-5-80 43.3% [35.7, 51.3]** | **87-1-62 58.0% [50.0, 65.6]** |

**Verdict.** 2-2-1-1 pulls P1 to a coin-flip on this package: **43.7% →
50.0%** without the ratchet and **42.0% → 50.7%** with it. CIs cover
50% and overlap the 1-1-1-1 arms (2211 r1 lo 45.0 vs mm-mm r1 hi 47.7).
The lift is the **opening**, not a ratchet interaction — 2211 baseline
and 2211 r1 are 150 vs 152 P1 wins. Search/eval were not touched.

The seed-block split is **not uniform**. 2211 r1 barely moves 300–449
(41.3% → 43.3%) and jumps 10300–10449 (42.7% → 58.0%). 2211 baseline is
flatter (47.3% / 52.7%). Treat 50.7% as the pooled figure, not as
“every deal is now fair.”

Same-deal vs mm-mm r1: 130/300 games keep the same winner, 170 flip;
net **+26 P1 wins** (95 newly won, 69 newly lost).

## Opening doubles (both 2211 arms)

The first four plies do not ratchet, so exactplay minimax plays the
**same opening** with or without the ratchet flag (P1 kinds 574 ADD +
26 MOVE on both logs).

| | P1 | P2 |
|---|---|---|
| both-ADD doubles | **274/300 (91.3%)** | 180–181/300 (~60%) |
| dest rings | outer 584, center 15, inner 1 | outer 505, center 48, inner 47 |
| touched origin on the double | **15/300 (5.0%)** | **48/300 (16.0%)** |
| P2 takes 0 as a reply to P1-on-0 | — | **0** (P1 almost never occupies 0 first) |

P1 **develops** (two outer ADDs). P2 **sometimes races** for the origin
on its reply double (ADD spawn, MOVE onto 0). They do not fight over
the center on the opening pair: P1 is not contesting 0, and P2’s race
is one-sided.

That race does **not** punish P1:

- 2211 r1: P2 takes 0 on the double in 48 games; **P1 wins 33, P2 14, 1
  draw**.
- 2211 baseline: same 48 openings; **P1 wins 39, P2 9**.

On seeds 10300–10449 + r1, that line is 20–3 for P1. Sitting on 0 after
only one piece is a liability (control-cascade + ratchet), not a steal
of the extra P1 development.

## Qualitative (cite log / game / seed / turn)

JSONL reconstructs every action (`kind`, `from`/`to`,
`played_as_opening`, `fight_gain`, `ratchet_kind`, `ratchet_tier`,
`played_as_ratchet`).

### [`h2h_n300_mm_r1`](h2h_n300_mm_r1.jsonl.gz) game 2 / seed 10301 / 39 turns

P1 ADD `n2` then MOVE `2→5` (develop, not origin). P2 ADD `n2` then
**MOVE `2→0` on the opening double**. P2 vacates 0 at t6, retakes t14,
vacates t28. First ratchet is P1 cascade ADD t33; t37 CONVERT outer
3-ring (`n14`); t39 P1 MOVE onto 0, **control**. P2’s reply-double
origin grab does not stick; P1 wins anyway.

### [`h2h_n300_mm_r1`](h2h_n300_mm_r1.jsonl.gz) game 11 / seed 310 / 31 turns

Typical P1 both-ADD (`n2`, `n8`). P2 ADD `n5` then MOVE onto 0. t7 P1
inner MOVE is an **opponent-won fight** (P2 first reward ADD, not
SWAP). Origin changes hands (P2 retakes t20, vacates t22). P1 CONVERT
`n11` t24 then lockout t31. **Comeback**: P2 was the first-ratchet
leader. Counterplay after the grab, not a punished opening.

### [`h2h_n300_mm_r1`](h2h_n300_mm_r1.jsonl.gz) game 6 / seed 10305 / 31 turns

The rare P1 race: ADD `n2` then **MOVE `2→0`**. P2 both-ADD outer. No
ratchet. P1 never leaves 0 (`never_left`), inner develops, **control
t31**. Sit-and-win from a 2-2-1-1 origin race — 15/300 P1 openings, not
the modal line.

## Open questions

1. Consecutive 1-action turns vs a 2-action first turn. Implemented as
   consecutive so ratchet/lockout apply per action and mean turns stay
   comparable. A cap=2 first turn is unwired.
2. Mover-won ratchet consuming the leftover (no third go). Not observed
   on the opening in this cell (0/300 interrupted).
3. Opponent interrupt forfeiting P1’s leftover while P2 still owes its
   double. Same: not hit on these deals.
4. `--actions 2` stacking with `--opening-double`. Not benched.
5. Seed-block split on 2211 r1 (43.3% vs 58.0%). Pooled 50.7% may
   overstate how even the rule is deal-by-deal. A second independent
   draw would help.
6. P2’s origin-on-double line looks like a search artifact (loses
   33–14 under r1, 39–9 without). If humans race for 0 on the reply
   double, the seat numbers here overstate P1’s edge under that plan —
   or understate it, if humans refuse the race that the bot plays 16%
   of the time.
7. Sit-and-win ticked **up** (10.5% → 15.2% without ratchet; 10.7% →
   12.8% with). 2-2-1-1 equalizes the seat and slightly increases
   camps. Not the goal if sit-and-win was the complaint.
8. Comebacks after the first ratchet also rose (18.3% → 26.6%). Could
   be the extra opening material, or the 10300-block deal mix.
