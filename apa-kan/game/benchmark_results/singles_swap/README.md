# Singles-swap move (stacked on PR #79 config B)

**Locked in:** adjacent-any (single-ring tile swaps with any adjacent
occupied tile, own or opponent) plus the one-move no-echo guardrail.
That is now the live default (`--singles-swap on`). `--singles-swap off`
restores the old no-swap rules. Comparison modes (`adjacent-own`, `any`,
`own`) and `--swap-no-just-moved` / `--swap-no-immediate-back` were
removed; no-echo is baked in whenever swaps are on.

The `h2h_*` logs below are a historical record of the comparison suite.
Do not overwrite them. The locked-in confirmation run is
[`h2h_b_locked_counterplay`](h2h_b_locked_counterplay.txt) (defaults,
counterplay, 100 games): P1 **50-3-47**, mean 71.20, never-left **25.0%**,
matching the earlier adj-any + no-back log.

Stacked on `cursor/ring-dist-channels-search-97b5` (PR #79). This package
measured a new full action: a **single-ring tile** swaps positions with
another occupied tile.

Do not overwrite committed `h2h_*` logs; quote numbers from those files.

## Liam's variants (historical)

Liam asked for a saboteur role for singles after PR #79's survey: on
IndependentMax the first center-taker is a coin flip, but ~40% of first
center pieces never leave. He phrased the two benchmark variants as
"singles can swap with any tile" and "singles can swap with any of your
tiles" right after "singles can swap position with an adjacent piece".

The comparison suite below used these CLI names. **Only adjacent-any
with no-swap-back was kept**; the live flag is `--singles-swap on|off`.

| Phrase | Historical mode | Historical CLI |
|---|---|---|
| adjacent piece / "any tile" | adjacent-any (now `On`) | `--singles-swap adjacent-any` |
| "any of your tiles" | adjacent-own (removed) | `--singles-swap adjacent-own` |
| unrestricted (comparison) | any / own (removed) | `--singles-swap any` / `own` |

## Rule choices

Saveable on `RuleSet` like the other rules. JSONL missing keys replay as
`off` / `false`.

- **Initiator:** current player's tile with `n_rings == 1`. Partner: occupied
  tile, filtered by mode. SWAP is a full action, spends **no** spawn token,
  and is **not** MOVE (`capture_draw_on_move` does not fire).
- **After SWAP:** initiator at `to`, partner at `from`. Taking the opponent's
  center seat is the intended saboteur play. Vacant center cannot be swapped
  onto (needs a partner); every first take in these logs is still MOVE.
- **Local-claim:** union of both landing nodes and their unique neighbors
  (`collect_local_removals_pair`). Cascade: full board. Acting capture mark
  is the initiator (now at `to`).
- **Control / ratchet / turn-gain / center-hold:** the same `finish_turn`
  pipeline after captures; occupancy at turn end is what scores.
- **Emblems / beads / braids:** recast after landing. Blocker does not
  block SWAP (no path).
- **`no_same_piece_twice`:** SWAP counts as moving the initiator; capture
  exception via `swap_removes_opponent`.
- **Search:** center-dislodging SWAPs are tactical (skip LMR, enter qsearch)
  even without removals. `cheap_order` prefers kicking an opponent off
  center (+2500/+1200). Reverse-swap penalty does **not** fire across
  players at the root (each `Searcher` tracks its own last move), which is
  why opponent echo shows up in the logs.
- **No-echo (baked in when swaps are on):** neither player may SWAP the
  same unordered tile-id pair on the immediately following move. The
  cooldown lasts one move. `--swap-no-immediate-back` / `--swap-no-just-moved`
  were comparison flags and are gone.

Logs: `SWAP a <-> b`, JSONL `kind=SWAP` with `partner` / `partner_owner`.
UI: click own single, then a highlighted partner (magenta). Branching
probe reports a SWAP line when the mode is on.

## Package

Same harness as PR #79: release `apa-kan-bench`, `--no-default-features`,
`--budget nodes:33000,iters:650`, `--games 10`, `--jobs 4`,
`--no-depth-probe`, `--zags on`, `--zag-mismatch sum`,
`--endgame-rule control-cascade`, `--cascade-turn-gain on`,
`--capture-mode local-claim`, `--no-same-piece-twice off`, `--pick search`,
`--pairings minimax:mcts,mcts:minimax`,
`--deck-matchups mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`,
`--seed 1`, `--independent-channels on` (config B: 7:5:4, IndependentMax
heavy ~10% braid rings). 10 × 2 × 5 = **100 games** per config.

Swap-off baseline is quoted from the already-committed PR #79 logs
[`../ring_dist_channels/h2h_b_imax_counterplay`](../ring_dist_channels/h2h_b_imax_counterplay.txt)
and
[`../ring_dist_channels/h2h_b_imax_enhanced`](../ring_dist_channels/h2h_b_imax_enhanced.txt).

Reproduce with [`run_h2h_shallow.sh`](run_h2h_shallow.sh). Center-holder
survey: [`survey_center.py`](survey_center.py).

| Log | Swap | Search | Guardrail | Games |
|---|---|---|---|---|
| [`h2h_b_adj_any_counterplay`](h2h_b_adj_any_counterplay.txt) | adjacent-any | counterplay | off | 100 |
| [`h2h_b_adj_own_counterplay`](h2h_b_adj_own_counterplay.txt) | adjacent-own | counterplay | off | 100 |
| [`h2h_b_any_counterplay`](h2h_b_any_counterplay.txt) | any | counterplay | off | 100 |
| [`h2h_b_own_counterplay`](h2h_b_own_counterplay.txt) | own | counterplay | off | 100 |
| [`h2h_b_adj_any_counterplay_noback`](h2h_b_adj_any_counterplay_noback.txt) | adjacent-any | counterplay | no-immediate-back | 100 |
| [`h2h_b_locked_counterplay`](h2h_b_locked_counterplay.txt) | on (live default) | counterplay | no-echo baked in | 100 |
| [`h2h_b_adj_any_enhanced`](h2h_b_adj_any_enhanced.txt) | adjacent-any | enhanced | off | 100 |
| [`h2h_b_adj_own_enhanced`](h2h_b_adj_own_enhanced.txt) | adjacent-own | enhanced | off | 100 |

Unrestricted `any`/`own` enhanced skipped; counterplay already showed those
modes are spammy.

## Head-to-head (quoted)

| Log | P1 W-D-L | Mean turns | Ends (lock / ctrl / LV / TL / elim) | Opp / self cap/game | Claims /game | Turn-gain | Center opp cap | Swaps /game | Center swaps /game |
|---|---|---|---|---|---|---|---|---|---|
| B off counterplay (#79) | 53-2-45 | 69.55 | 7 / 90 / 1 / 2 / 0 | 4.03 / 5.25 | 0.00 | 0.36 | 0.34 | 0 | 0 |
| adj-any counterplay | 44-2-54 | 68.75 | 6 / 92 / 0 / 2 / 0 | 4.49 / 6.00 | 0.02 | 0.40 | 0.32 | **4.48** | **2.06** |
| adj-own counterplay | 51-1-48 | 62.10 | 6 / 93 / 0 / 1 / 0 | 3.95 / 4.65 | 0.03 | 0.17 | 0.23 | 2.23 | 1.57 |
| any counterplay | **60-3-37** | 79.72 | 11 / 86 / 0 / 3 / 0 | 5.72 / 5.42 | 0.01 | 0.48 | **0.55** | **12.69** | 3.54 |
| own counterplay | 54-3-43 | 63.50 | 6 / 90 / 1 / 2 / 1 | 4.13 / 4.80 | 0.01 | 0.35 | 0.26 | 5.33 | 2.10 |
| adj-any + no-back | 50-3-47 | 71.20 | 10 / 86 / 2 / 2 / 0 | 4.13 / 4.95 | 0.01 | 0.27 | 0.35 | 5.55 | 2.28 |
| locked default | 50-3-47 | 71.20 | 10 / 86 / 2 / 2 / 0 | 4.13 / 4.95 | 0.01 | 0.27 | 0.35 | 5.55 | 2.28 |
| B off enhanced (#79) | 48-3-49 | 63.50 | 3 / 93 / 1 / 3 / 0 | 3.32 / 4.85 | 0.01 | 0.22 | 0.14 | 0 | 0 |
| adj-any enhanced | 49-4-47 | 62.94 | 4 / 92 / 0 / 4 / 0 | 3.62 / 4.60 | 0.01 | 0.21 | 0.24 | 5.67 | 2.88 |
| adj-own enhanced | 38-5-57 | 63.58 | 3 / 90 / 0 / 5 / 2 | 3.48 / 4.85 | 0.00 | 0.21 | 0.20 | 2.12 | 1.52 |

`adjacent-any` does not lengthen games vs B and does not raise same-turn
center pops (0.32 vs 0.34). The extra counterplay is the SWAP itself.
Unrestricted `any` is the only mode that lifts center opp caps (0.55) and
P1 win rate (60–3–37), at the cost of 12.69 swaps/game and three
turn-limit draws.

## Center-holder survey

From [`survey_center.py`](survey_center.py) on the JSONL (center = node 0).
SWAP onto/off center transfers occupancy (initiator lands at `to`).

| Log | First take | Still / never left | First-taker side wins | Still-and-that-side-wins | Vacates | Center opp caps | Swaps / center |
|---|---|---|---|---|---|---|---|
| B off counterplay (#79) | 100/100 @ t5.4 | 48.0% / **40.0%** | 49.0% | 37/48 = 77% | 145 | 34 | 0 / 0 |
| adj-any counterplay | 100 @ t5.6 | **34.0% / 25.0%** | 51.0% | 27/34 = 79% | 313 | 32 | 4.48 / 2.06 |
| adj-own counterplay | 100 @ t5.7 | 48.0% / 39.0% | 56.0% | 40/48 = 83% | 266 | 23 | 2.23 / 1.57 |
| any counterplay | 100 @ t7.0 | **24.0% / 16.0%** | 46.0% | 18/24 = 75% | 515 | 55 | 12.69 / 3.54 |
| own counterplay | 100 @ t5.2 | 41.0% / 36.0% | **62.0%** | 34/41 = 83% | 305 | 26 | 5.33 / 2.10 |
| adj-any + no-back | 100 @ t5.3 | 31.0% / 25.0% | 45.0% | 24/31 = 77% | 382 | 35 | 5.55 / 2.28 |
| locked default | 100 @ t5.3 | 31.0% / 25.0% | 45.0% | 24/31 = 77% | 382 | 35 | 5.55 / 2.28 |
| B off enhanced (#79) | 99 @ t9.1 | 48.5% / **43.4%** | 49.5% | 29/48 = 60% | 158 | 14 | 0 / 0 |
| adj-any enhanced | 100 @ t9.3 | 33.0% / 29.0% | **64.0%** | 29/33 = 88% | 454 | 24 | 5.67 / 2.88 |
| adj-own enhanced | 100 @ t9.4 | 39.0% / 34.0% | 59.0% | 32/39 = 82% | 346 | 20 | 2.12 / 1.52 |

**Center CLAIM stays unused** (0 in every swap log). First take is MOVE in
100/100 games on every config — SWAP cannot occupy an empty center.

`adjacent-any` is the mode that actually cuts never-left (40% → 25%
counterplay, 43% → 29% enhanced). `adjacent-own` cannot yank an opponent
holder (all 157 adj-own-counterplay center swaps had `partner_owner ==
player`); never-left stays ~40%. Unrestricted `any` matches main's 16%
never-left, by teleporting.

## Qualitative survey

**Sit-and-hold is still there, just rarer.** adj-any counterplay mcts vs
minimax bead vs bead game 7: P2 MOVE onto center at t12, zero vacates /
swaps / captures, control hold at t124. 17 of the 25 never-left games in
that log had no SWAP at all.

**Singles do act as saboteurs under adjacent-any.** 67/100 games used at
least one SWAP; 46 had a center SWAP; 79 SWAPs kicked an opponent off
node 0. Example: mcts vs minimax braid vs bead game 8, P2 MOVE 2→0 at t4
with a 3-ring; P1's single SWAP 21→0 at t19 knocks it off. P2 still wins
control-5 at t26 — the kick happened, it just did not flip the match.
Another: braid vs braid game 2, P2 SWAP 16→0 at t18 kicking P1's 3-ring,
later P1 SWAP 9→0 at t69 with two captures.

Games feel less like "shuffle until the center holder wins" under
adjacent-any: vacates 145 → 313 and 454 distinct center occupants across
the 100 games (B counterplay was 145 vacates). The win-type mix stays
control-heavy. First-taker side win rate stays a coin flip on counterplay
(49% → 51%). Enhanced adjacent-any is the odd one: never-left drops
43% → 29% but first-taker *side* wins jump 50% → 64% (still-and-that-side
88%). Counterplay is the better personality for Liam's goal.

**Degenerate pattern: opponent echo / ping-pong.** adj-any counterplay
minimax vs mcts bead vs braid game 6 (36 swaps, 25 center, 19 of the same
tile pair): t10 P2 SWAP 2↔1 (ids 40↔1), t11 P1 echoes, t14/t15 echo,
t20–t23 echo, then the same two singles bounce on the center (t25–t39
`1↔0` / `3↔0`). P1 wins lockout at t61. Seven games had consecutive
same-pair echoes (33 events); 16 games re-swapped a pair within two
turns.

`--swap-no-immediate-back on` (adj-any counterplay noback) zeros those
echoes (0 consecutive, 0 within-two-turns) without moving never-left
(still 25%). Swap volume does not drop (4.48 → 5.55): bots pick a
different partner. One 63-swap / 38-center-swap game remains (mcts vs
minimax braid vs braid game 3, 200 turns). That no-echo rule is now
baked in whenever swaps are on.

**adjacent-own is own-piece rotation, not saboteur.** Center SWAP partner
is always own. braid vs braid minimax vs mcts game 4: 45 swaps / 44
center, P1 bouncing a single with its own 3-ring on 0↔21 at t69–t73.
Never-left 39%.

**Unrestricted `any` is teleport spam.** 12.69 swaps/game, games with 50–68
swaps, 37 games with consecutive same-pair echo, 3 turn-limit draws.
Never-left 16% is the main-like number Liam wanted, but the games do not
look like Apa Kan with a saboteur single — they look like pieces
teleporting. `own` unrestricted has a 75-swap game (mcts vs minimax bead
vs braid game 9) and first-taker side wins **62%** (the holder rotates
itself to safety).

## Open design questions (resolved)

Locked in: adjacent-any + one-move no-echo, on by default.
`--singles-swap off` is the only remaining toggle (old no-swap rules).

- SWAP cannot take a vacant center. First takes remain MOVE.
- No-echo kills the ping-pong (33 consecutive echoes → 0) without costing
  the 40%→25% never-left cut.
- `--swap-no-just-moved` was not kept; the abuse was pair-echo, not
  "I moved then swapped the same piece."
- `adjacent-own` does not give singles a saboteur role against an
  opponent holder.
- Unrestricted `any` matches main's 16% never-left only by spamming SWAP.
- CLAIM-on-center is still dead (0 in 700 new games here, 0 in PR #79).
- Search reverse-swap penalty is per-`Searcher`, so opponent echo is a
  rules problem, not an LMR/qsearch miss. Center SWAPs are already
  tactical so they are not pruned.
- Light H2H still uses `--zag-mismatch sum` on IndependentMax.
