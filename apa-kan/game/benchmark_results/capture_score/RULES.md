# Capture-score ruleset (`--ruleset capture-b`)

One switch sets the whole capture-score game. Flags given after it override single fields, for example `--ruleset capture-b --repetition-ends off`. Control stays the live default (`--ruleset control` resets the fields below).

| Field | capture-b | Why |
|---|---|---|
| objective | capture-score | 1 point per opponent tile captured; self-captures score nothing. Control, cascade and ratchet wins are off. |
| score target | 5 | First to 5 with a strict lead wins. At the turn limit (200), the higher score wins and a tie is a draw. |
| unlimited spawn | on | ADD does not spend spawn tokens. Captured tiles leave the board through the routing below. |
| opening | 1-1-1-1 | Matches the B sweep cells (#105, #106). |
| graveyard | on | Graveyard tiles come back with RECOVER, which costs one action. |
| mark routing | B | See below. |
| no-immediate-return | on | See below. |
| repetition-ends | on | See below. |

## Mark routing B

The acting tile's mark decides where every tile removed by the action goes. The acting tile is the one moved, added, SWAP-initiated or converted. CLAIM and Pass count as triangle.

| Acting mark | Own tile captured | Opponent tile captured |
|---|---|---|
| dot | your graveyard | your graveyard (you own it) |
| triangle | opponent's graveyard (they own it) | your hand (you own it) |

## No-immediate-return

A tile may not MOVE (or initiate a SWAP) back onto the node it left on its own last move, on its owner's next turn or later in the same turn. There is no capture exception. A SWAP partner is not blocked, and a captured tile's record is dropped.

## Repetition ends the game on points

The 3rd occurrence of the same visible position (board, benches, graveyards, scores, side to move, return blocks), checked at turn boundaries, ends the game. The leader wins and a tie is a draw (`EndReason::Repetition`).

## Overtime: next capture wins

Once both players are at 4 (target − 1), the next opponent capture by either side wins on the spot. A self-capture never ends it, because it scores nothing.

This is already how the score rule works: an opponent capture takes the capturer to 5 with a strict lead. No separate flag is needed.
- A triangle opponent capture going to your hand under routing B still counts as the winning capture.
- The score check runs inside `apply`, so search treats the position as terminal.
- Tests:
  - `capture_b_tied_match_point_next_opponent_capture_wins`;
  - `capture_b_tied_match_point_self_capture_does_not_end_the_game`;
  - `minimax_takes_the_tied_match_point_capture_under_capture_b`.
- In every committed 150-game log, the first opponent capture after a tied 4-4 was the game's last action.

## Self-capture penalty (`--self-capture-penalty off|always|match-point`)

Default off, and `capture-b` leaves it off. A self-captured tile scores 1 point for the opponent, per tile, so a multi-tile self-capture gives several points. Routing B is unchanged: a triangle self-capture still goes to the opponent's graveyard and a dot self-capture to your graveyard.

- `always`: every self-captured tile scores for the opponent.
- `match-point`: only while the self-capturer is at `score_target - 1` before the action. At 4-4 both players are covered, but a player at 3 can still self-capture freely while their opponent sits at 4. I chose this over "while either player is at target - 1" because the penalty then belongs to the player whose own score made the self-capture a safe stalling move, and it never hands a win to someone just for the other player being close. Both readings are the same at a tied 4-4, where the standoff happens.

Both scores from one action are added together, and the match-point check uses the score from before the action. Both sides can therefore reach the target on the same action. A win still needs the target and a strict lead, so 5-5 plays on. A penalty point that reaches the target with a strict lead is a normal `EndReason::Score` win for the opponent. The score check runs inside `apply`, so search sees it as terminal; a test checks that the evaluator scores it as a loss.

## What was removed

These rules are gone from `RuleSet`, the CLI and `run_h2h.sh`:
- the majority-spawn bonus and its cell sets;
- the sparse-board capture cascade;
- capture-to-capturer;
- mark routing modes A and A-discard (and `CaptureDest::Discard`).

Their committed logs stay in this folder as history, alongside `FINDINGS.md` (#94), `GRAVEYARD.md` (#103), `MARK_ROUTING.md` (#105) and `NO_RETURN.md` (#106). Replaying one now fails with "log uses removed rule …" instead of replaying wrong.

## Results

`run_h2h.sh capture_b_mm`: 150 games, minimax vs minimax, seed 300 (75 deals × 2 seats), release, `--no-default-features`, SUM. Logs: `h2h_capture_b_mm.{jsonl.gz,txt}`.

| Cell | n | P1 W-D-L | P2 W-D-L | P1 WR | mean / med turns | opp / self caps per game | mean score P1 / P2 (margin) | end reasons | repetition | comebacks | dest C/I/S/O % | capture C/I/S/O % | lead tail med / ≥10 / loser counterplay | RECOVER/game |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| mark B (#105) | 150 | 65-23-62 | 62-23-65 | 0.433 | 98.1 / 74.5 | 8.08 / 13.93 | 4.03 / 4.05 (abs 1.47) | score 116, draw turn_limit 23, turn_limit 7, locked_out 4 | 8 rep≥3 (avg max 1.85) | 88/127 (69.3%) | 3.1/21.5/52.0/23.4 | 2.6/12.7/68.8/15.8 | 0 / 48 / 34/125 | 14.5 |
| B + no-return (#106) | 150 | 69-28-53 | 53-28-69 | 0.460 | 101.7 / 76.0 | 7.98 / 14.81 | 4.09 / 3.89 (abs 1.53) | score 110, draw turn_limit 28, locked_out 7, turn_limit 5 | 7 rep≥3 (avg max 1.60) | 79/122 (64.8%) | 2.7/21.1/52.8/23.4 | 2.3/12.7/70.3/14.7 | 0.0 / 50 / 35/120 | 15.6 |
| **capture-b** | 150 | 69-28-53 | 53-28-69 | 0.460 | 99.4 / 74.0 | 7.97 / 13.70 | 4.09 / 3.89 (abs 1.52) | score 109, draw turn_limit 26, locked_out 7, turn_limit 4, repetition 2, draw repetition 2 | 4 rep≥3 (avg max 1.14) | 79/122 (64.8%) | 2.7/21.4/52.2/23.7 | 2.3/12.7/70.2/14.7 | 0.0 / 50 / 35/120 | 14.5 |

- **It plays like B + no-return.** 143 of 150 games are move for move identical to that cell. Repetition-ends fires 4 times (2 won, 2 drawn at 4-4) and changes 7 games.
- **Seats:** P1 0.460 (69-53 decided, binomial p = 0.17). The #106 seat analysis showed this split is noise.
- **Pace:** mean 99.4 turns, median 74. That is the same as B, and still well behind t5 (median 54.5).
- **Comebacks and snowball:**
  - Comebacks are 64.8% of decided games, and the lead-tail median is 0.
  - The first player to lead by 2 won 72 of 136, close to a coin flip, so there is no snowball.
  - Triangle share of opponent captures is 0.57 (winners 0.54, losers 0.59). 54.3% of opponent captures hit a tile that moved on its owner's last turn, with 0 undo moves.
- **Tied match point:**
  - 91 games reach 4-4. They wait a median of 36 turns (mean 55.8) after it, and 26 still reach the turn cap.
  - After 4-4, 61 games are won by the next capture, 26 are drawn at the cap, 2 are drawn on repetition and 2 end in lockout.

### Qualitative: does overtime create attacking play at 4-4?

No. Because "next capture wins" was already true at 4-4, the standoff looks exactly like B + no-return.
- **It is a stall, not an exact loop.**
  - In the 48 standoffs that last 30 or more actions, 82% of MOVEs are distinct.
  - The bots shuffle, and spend tempo on RECOVER (14.5% of standoff actions) and self-captures (0.13 per action). Only 23 of those 48 standoffs end in a capture.
  - Example, seed 10341 at 4-4 (t164–t177): P2 dot-ADDs into a self-capture at 23, P2 sends a triangle self-capture to P1's graveyard at 16, both RECOVER, and P1 walks tiles 24→4, 12→18, 13→17, 18→21. Nobody moves into contact, and it is drawn at the cap.
- **When it does resolve, it is a multi-capture out of quiet play, not a sustained attack.**
  - Seed 10314 (t58): P2 equalizes to 4-4 with a triangle MOVE 4→24, taking P1's 23 into P2's hand, but that leaves 24 exposed. P1 dot-ADDs at 23, removes 21 and 24, and wins 6-4 on the next action.
  - Seed 307 (t68): P1's dot MOVE 2→7 removes 7, 8 and 10 and wins 5-4.
- **Verdict:** the remaining problem is incentive, not detection. At 4-4 any exposure loses, and waiting costs nothing. A rule has to make waiting cost something (a match-point clock), make the stall moves cost something (a self-capture or RECOVER at match point scoring for the opponent), or make attacking safer.

### Self-capture penalty cells

150 games each; same bench args as `capture-b` plus `--self-capture-penalty`. Data: `h2h_capture_b_penalty_{always,matchpoint}_mm.{jsonl.gz,txt}`.

| Cell | P1 W-D-L (seat p) | mean / med turns | opp / self caps per game | mean final score | end reasons | comebacks | first to +2 won | RECOVER/game |
|---|---|---|---|---|---|---|---|---|
| capture-b | 69-28-53 (p=0.17) | 99.4 / 74.0 | 7.97 / 13.70 | 4.09 / 3.89 | score 109, draw turn_limit 26, other 15 | 79/122 (64.8%) | 72/136 | 14.5 |
| + penalty always | 68-10-72 (p=0.80) | 64.1 / 47.0 | 8.74 / 7.55 | 8.10 / 8.19 | score 140, draw turn_limit 10 | 22/140 (15.7%) | 21/23 | 8.8 |
| + penalty match-point | 74-17-59 (p=0.23) | 90.6 / 69.5 | 8.34 / 11.81 | 4.75 / 4.55 | score 119, draw turn_limit 16, other 15 | 94/133 (70.7%) | 62/129 | 12.8 |

| Cell | penalty pts/game | lost on own penalty | winner needed gifted pts | tied 4-4 (wait mean/med, hit cap) | tied at ≥5-5 (wait med) | 1-for-1 trade actions | max lead ≤1 |
|---|---|---|---|---|---|---|---|
| capture-b | 0 | 0 | 0 | 91 (55.8 / 36, 26) | 0 | 0 | — |
| always | 7.55 | 0 | 90/140 | 85 (50.7 / 38, 10) | 68 (40) | 1085 | 127/150 |
| match-point | 0.95 | 0 | 11/119 | 100 (38.8 / 16, 15) | 63 (29) | 98 | — |

Here "1-for-1 trade actions" means actions where both players scored. "Lost on own penalty" counts games where the loser's own self-capture handed over the winning point. Search never does this, because the penalty win is terminal.

**always.** Self-captures don't go away. They drop from 13.7 to 7.6 per game, and the rate per action barely moves (0.13 to 0.12). What changes is that they get paired with an opponent capture. The usual move becomes a MOVE or ADD that takes one enemy tile and one of your own, which scores +1/+1. Scores climb in lockstep (in 127 of 150 games nobody ever leads by 2), and 68 games pass 5-5 still tied. Seed 302 climbs 6-6 → 12-12 through one trade every 3-4 plies and ends 14-13 on a 2-enemy + 1-own capture. So play is contact-heavy, and every score change comes from a capture, but it is a trading race rather than attacking play. Leads are always 1 and short-lived, which is why comebacks drop to 15.7%. Games are about a third shorter and turn-limit draws fall from 26 to 10. The standoff shifts from MOVE toward ADD (ADD 17% → 31% of actions). RECOVER rises a little at the standoff (15% → 18%) but falls per game; I found no RECOVER-as-pass stall.

**match-point.** The 4-4 wait halves (median 36 → 16) and the seats are the most even of the three. But the penalty is only live at exactly `target - 1`. A 1-for-1 trade at 4-4 lifts both players to 5-5, where the penalty is off again and the next opponent capture wins. That is the old 4-4 standoff one point higher: 63 games reach 5-5, and 16 end in turn-limit draws there. Seed 10303 shuffles MOVEs at 5-5 from turn 177 to 200, with free self-captures at turns 180 and 200. Making the penalty apply at `score >= target - 1` would close this gap. That variant has not been run.
