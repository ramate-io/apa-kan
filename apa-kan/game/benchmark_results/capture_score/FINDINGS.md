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

Light package: release, `--no-default-features`, `nodes:33000,iters:650`, exactplay, zag-mismatch sum, `--jobs 4`. Full cells are 150-game mm-mm (seed 300) and 100-game mm-mcts (seed 10300). Smokes below are n=4 (seed 1) used only to pick the next lever.

### Smoke (n=4, mm-mm, seed 1)

| Arm | n | P1 W-D-L | mean/med turns | caps/game | score mean P1/P2 (abs margin) | how games end |
|---|---|---|---|---|---|---|
| control (live) | 4 | 1-0-3 | 59.5 / 62 | 6.75 | — | 3 control, 1 lockout |
| capture-score base (`eval_capture_score=0`) | 4 | 2-1-1 | **200 / 200** | 8.00 | 4.00 / 4.00 (1.00) | **0 target, 4 turn-limit** |
| + `eval_capture_score=80` | 4 | 2-0-2 | 192 / 200 | 7.25 | 3.25 / 4.00 (1.75) | 1 target, 3 turn-limit |
| + `eval_capture_score=400` | 4 | 0-1-3 | 200 / 200 | 4.75 | 1.75 / 3.00 (1.25) | 0 target, 4 turn-limit (more timid) |
| + majority_spawn_bonus | 4 | 2-0-2 | **73 / 79** | 9.25 | 4.75 / 4.50 (2.75) | **4/4 score-target** |
| + capture_cascade | 4 | 2-0-2 | 119 / 124 | 5.50 | 2.75 / 2.75 (3.50) | 1 score, 1 lockout, 2 turn-limit |
| + unlimited_spawn | 4 | 1-1-2 | 142 / 166 | 9.75 opp, **58.5 self** | 4.50 / 5.25 (1.75) | 2 score, 2 turn-limit |

Full 150-game arms are running.

### Full H2H

| Arm | Pairing | n | P1 W-D-L | P1 WR | mean/med turns | caps/game | score margin | target vs turn-limit | comebacks |
|---|---|---|---|---|---|---|---|---|---|
| control (live) | mm-mm | _running_ |  |  |  |  | — | — |  |
| capture-score base | mm-mm |  |  |  |  |  |  |  |  |
| + majority_spawn_bonus | mm-mm |  |  |  |  |  |  |  |  |
| + capture_cascade | mm-mm |  |  |  |  |  |  |  |  |
| + unlimited_spawn | mm-mm |  |  |  |  |  |  |  |  |

## Qualitative (smoke)

Base scoring with weight 0 does not hunt captures. Smoke_base game seed=1: opening ADDs on outer spawns (t1–t6), first capture at **t23** (P1 MOVE to node 1, takes P2 off center). End is **t200 TurnLimit, 5–4**. Late ply is MOVE ping-pong on the inner ring (t196–t200, score stuck 5–4). Dest mix: 559 outer / 217 inner / 24 center over 800 actions. Zero CLAIM/SWAP/CONVERT.

`eval_capture_score=400` made this worse (fewer opponent captures). Do not bake a large score weight from this n=4.

`majority_spawn_bonus` is the first lever that actually finishes games (all four hit target 6 around turn 46–88). Mean tokens/side 7.30 vs 0.95 on base — the extra spawn token is spent, so more pieces hit the board and get captured.

`capture_cascade` does fire (13 capture-ratchets, 3 CONVERT / 2 SWAP / 8 extra ADD) but two of four games still hit the cap.

`unlimited_spawn` returns every capture to the owner; self-captures explode (58/game) and one game had max_repeat=51 (looping).

## Honest take

Pending the 150-game cells. Smoke says **bare capture-score is not yet a better game** — it stalls, bots shuffle on the outer ring, and they stop one point short of the target. Majority-of-former-control as a spawn bonus looks like the bones of a scoring game that still has a reason to fight the center. Control remains shorter and more decisive.
