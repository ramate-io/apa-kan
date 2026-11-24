# Capture-score ruleset (`--ruleset capture-b`)

One switch sets the whole capture-score game. Flags given after it override single fields, for example `--ruleset capture-b --repetition-ends off`. Control stays the live default (`--ruleset control` resets the fields below).

| Field | capture-b | Why |
|---|---|---|
| objective | capture-score | 1 point per opponent tile captured; self-captures score nothing. Control, cascade and ratchet wins are off. |
| score target | 5 | First to 5 with a strict lead wins. At the turn limit (200), the higher score wins and a tie is a draw. |
| unlimited spawn | on | ADD does not spend spawn tokens. Captured tiles leave the board through the routing below. |
| opening | 1-1-1-1 | The 2-2-1-1 opening gave P2 the first capture almost every game. |
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

## What was removed

These rules are gone from `RuleSet`, the CLI and `run_h2h.sh`:
- the majority-spawn bonus and its cell sets;
- the sparse-board capture cascade;
- capture-to-capturer;
- mark routing modes A and A-discard (and `CaptureDest::Discard`).

Their committed logs stay in this folder as history, alongside `FINDINGS.md` (#94), `GRAVEYARD.md` (#103), `MARK_ROUTING.md` (#105) and `NO_RETURN.md` (#106). Replaying one now fails with "log uses removed rule …" instead of replaying wrong.

## Results

Pending: `run_h2h.sh capture_b_mm` (150 games, minimax vs minimax, seed 300).
