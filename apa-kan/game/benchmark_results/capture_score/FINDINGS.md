# Capture-score overnight experiment

Live default stays the control-cascade game. This cell adds `--objective capture-score` and three independent toggles, all off by default.

Reproduce with [`run_h2h.sh`](run_h2h.sh). Survey: [`survey.py`](survey.py). Quote numbers from the committed `*.txt` / JSONL. P1 win-rate CI is Wilson 95% on wins/n.

## Rules as implemented

| Flag | Default | Effect |
|---|---|---|
| `--objective control\|capture-score` | `control` | Capture-score: 1 point per opponent piece captured. Control occupancy, cascade-drop wins, and center-fight ratchets do not win. `control_win` is left on so historical JSONL and the live default are unchanged; `RuleSet::control_wins_active()` is false under capture-score. |
| `--score-target 1..64` | `6` | First-to-N opponent captures. 6 is more than an opening skirmish (2–3) and less than wiping an 11-tile bench. Benches keep `turn_limit=200`. |
| `--majority-spawn-bonus on\|off` | off | At `finish_turn`, if you occupy strictly more of the former 9 control cells than the opponent (and at least one), gain one spawn token. Once per completed turn. Tied or empty occupancy awards nothing. |
| `--capture-cascade on\|off` | off | If this action captured ≥1 opponent piece and the board then has ≤8 occupied nodes, seat the capturer for the next action as a `RatchetKind::Capture` reward. Same 1st/2nd+ SWAP/FLIP ladder as live (`cascade_winner_swap` still gates the loosened verbs). Ignored under control. |
| `--unlimited-spawn on\|off` | off | Captured tiles return to the owner's bench (`CaptureDest::Owner`). ADD does not spend tokens; usable budget is bench length. Occupying spawn nodes is how you starve the opponent of landing spots. |

Timed games: `Game::resolve_timeout()` compares capture scores (higher wins, tie is a draw with `EndReason::TurnLimit`). The desktop app still has no total game clock (RFC follow-up); the hook is there. Untimed / benches end at the score target or the turn limit.

### Control-point dependencies

- **Control occupancy wins / cascade drops / low-value pair check** — gated off via `control_wins_active()` and `low_value_applies()`.
- **Center-fight ratchet** — skipped under capture-score. `--capture-cascade` reuses `seat_ratchet` on a sparse opponent capture instead.
- **FLIP / loosened SWAP** — still require a ratchet reward + `cascade_winner_swap`. Base scoring has no ratchets, so FLIP is off unless `--capture-cascade` is on.
- **Claim action** (local-claim of overwhelmed tiles) is not a control-point mechanic; it stays and scores 1 if the claimed tile is an opponent's.
- **Spawn nodes, 2-2-1-1 opening, tokens, local captures, swaps, braid-only-exact** stay.

Search: terminals include `EndReason::Score`. Leaf term `eval_capture_score * (my − opp)` is a per-bot TOML / `--search` overlay, default 0. No capture tactics, pick biases, or scripted hints.

JSONL missing `objective` / toggles replay as control / off. Score-0 Zobrist hashes match live control.

## Package

Release `apa-kan-bench`, `--no-default-features`, `--budget nodes:33000,iters:650`, `--search-profile exactplay`, `--zag-mismatch sum`, `--jobs 4`, `--no-depth-probe`, `--same-deck on`, `--pick search`. mm-mm uses `--pairings minimax:minimax,minimax:minimax --games 75` (150 games, seed 300). mm-mcts uses `--pairings minimax:mcts,mcts:minimax --games 50` (100 games, seed 10300).

## Results

_Filled after the light H2H runs._

| Arm | Pairing | n | P1 W-D-L | P1 WR | mean/med turns | caps/game | score margin | target vs turn-limit | comebacks |
|---|---|---|---|---|---|---|---|---|---|
| control (live) | mm-mm |  |  |  |  |  | — | — |  |
| capture-score base | mm-mm |  |  |  |  |  |  |  |  |
| + majority_spawn_bonus | mm-mm |  |  |  |  |  |  |  |  |
| + capture_cascade | mm-mm |  |  |  |  |  |  |  |  |
| + unlimited_spawn | mm-mm |  |  |  |  |  |  |  |  |

## Qualitative

_Read from JSONL samples after the first completed arms._

## Honest take

_Pending benches._
