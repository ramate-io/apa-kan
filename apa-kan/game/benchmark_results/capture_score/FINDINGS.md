# Capture-score overnight experiment

Live default stays the control-cascade game. This cell adds `--objective capture-score` and three independent toggles, all off by default.

Reproduce with [`run_h2h.sh`](run_h2h.sh). Survey: [`survey.py`](survey.py). Quote numbers from the committed `*.txt` / JSONL. P1 win-rate CI is Wilson 95% on wins/n.

## Rules as implemented

| Flag | Default | Effect |
|---|---|---|
| `--objective control\|capture-score` | `control` | Capture-score: 1 point per opponent piece captured. Control occupancy, cascade-drop wins, and center-fight ratchets do not win. `control_win` is left on so historical JSONL and the live default are unchanged; `RuleSet::control_wins_active()` is false under capture-score. |
| `--score-target 1..64` | `6` | First-to-N opponent captures. 6 is more than an opening skirmish (2–3) and less than wiping an 11-tile bench. Benches keep `turn_limit=200`. |
| `--majority-spawn-bonus on\|off` | off | At `finish_turn`, if you occupy strictly more of `--majority-spawn-set` than the opponent (and at least one), gain one spawn token. Once per completed turn. Tied or empty occupancy awards nothing. |
| `--majority-spawn-set control-set\|inner\|center` | `control-set` | Cells the majority bonus counts. `control-set` is the former 9 (center + inner). `inner` is the 8 petals (center does not count). `center` requires occupying the origin and a strict control-set majority. Missing JSONL replays as control-set. |
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

JSONL missing `objective` / toggles replay as control / off. Missing `majority_spawn_set` replays as control-set. Score-0 Zobrist hashes match live control.

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

All five 150-game mm-mm cells plus majority+cascade and majority+unlimited are in. Control / majority / base / cascade MCTS are in. Inner-trigger sweep is running; center trigger, target 4/8, weight cells, and unlimited MCTS follow.

### Smoke concentration (dest share of all actions)

Rings: center (node 0), inner (8 petals), spawn (8 middle), outer (8). Control's "middle ring" in the request is spawn.

| Arm | center | inner | spawn | outer | inner MOVE that captured opp |
|---|---|---|---|---|---|
| control | 5.9% | 21.4% | 47.9% | 24.8% | 15.7% (8/51) |
| capture-score base | 3.0% | 27.1% | 32.8% | **37.1%** | 3.7% (8/217) |
| + weight 80 | 3.5% | 22.1% | 34.9% | 39.5% | 6.5% (11/170) |
| + weight 400 | 3.0% | **37.4%** | 28.4% | 31.2% | **1.3% (4/299)** |
| + majority_spawn_bonus | 4.7% | 23.6% | **54.7%** | 16.9% | 10.3% (7/68) |
| + capture_cascade | 3.3% | 20.3% | 43.5% | 32.8% | — |
| + unlimited_spawn | 1.9% | 17.3% | **62.0%** | 18.7% | — |

Capture locations (smoke): control 10/30/40/20% center/inner/spawn/outer. Base scoring 3/22/43/32. Majority 8/21/49/21 — still spawn-heavy, but center captures 7.9% vs 3.1% on base. Piece usage stays 2-ring and 3-ring dominant; unlimited is the outlier (triangle marks 73%, 1-rings 15%).

### Why `eval_capture_score=400` looks timid

The leaf is `eval_capture_score * (my − opp)`. That term is **symmetric**: taking a piece is +400, getting recaptured is −400. Qsearch / stand-pat therefore prices a recapturable capture as a wash on the score channel, then subtracts hanging the capturer (`-22 * at_risk` plus losing the piece). Existing material already pays for the trade, so the extra 400 does not create initiative — it punishes being the one who opens contact.

Smoke (n=4, same seeds) matches that, and it is **not** "stay on the outer ring":

- Opponent captures drop 8.00 → 4.75/game; self-captures rise 8.25 → 10.25.
- Inner destinations *rise* (27.1% → 37.4%) while inner MOVEs that actually take an opponent piece fall 3.7% → **1.3%** (4 capturing inner MOVEs vs 295 that just park).
- Recapture-within-2 is similar (15.6% base vs 21.1% at 400) — they are not getting recaptured more often; they **stop initiating**.
- P1 leads on 0.1% of plies (vs 29.9% at weight 0). Seed=10002 at 400: P1 scores **0–3** and never captures.
- Seed=1 at 400: first capture is still t4 (P2 MOVE to outer 4), then the game stalls at 2–3 after t62. Base seed=1 reached 5–4 with a t23 inner capture on node 1.

Do not bake 400. Weight 80 is only barely less stalled (1/4 target wins). Tune from H2H on the majority lever, not by adding capture tactics.

### Full H2H

| Arm | Pairing | n | P1 W-D-L | P1 WR | mean/med turns | caps/game | score margin | target vs turn-limit | comebacks |
|---|---|---|---|---|---|---|---|---|---|
| control (live) | mm-mm | 150 | 75-3-72 | 0.500 [0.421, 0.579] | 49.0 / 40.5 | 6.48 | — | 97 control, 39 lockout, 5 elim, 6 LV, **3 TL draws** | n/a |
| capture-score base | mm-mm | 150 | **40-27-83** | 0.267 [0.202, 0.343] | **165.7 / 200** | 6.00 | P1 2.57 / P2 3.43 (abs 2.00) | **23 target / 86 TL wins / 27 TL draws** (113/150 hit cap) | 35/123 (28.5%) |
| + majority_spawn_bonus | mm-mm | 150 | 58-24-68 | 0.387 [0.312, 0.467] | **123.4 / 108.5** | 8.56 | 4.13 / 4.43 (abs 2.07) | **76 target / 38 TL wins / 24 TL draws** | **69/126 (54.8%)** |
| + capture_cascade | mm-mm | 150 | **30-18-102** | 0.200 [0.144, 0.271] | 139.6 / 200 | 4.23 | 1.31 / 2.93 (abs 2.58) | 23 target / 64 TL wins / 18 TL draws / **42 lockout** | 30/132 (22.7%) |
| + unlimited_spawn | mm-mm | 150 | **72-25-53** | 0.480 [0.402, 0.559] | **95.0 / 65.5** | 9.93 opp / **25.8 self** | 5.03 / 4.90 (abs 1.60) | **113 target / 8 TL wins / 25 TL draws** | **74/125 (59.2%)** |
| + majority + cascade | mm-mm | 150 | 54-17-79 | 0.360 [0.288, 0.439] | 129.3 / 131.0 | 6.25 | 2.77 / 3.49 (abs 2.83) | 48 target / 49 TL wins / 17 draws / 34 lockout | 47/133 (35.3%) |
| + majority + unlimited | mm-mm | 150 | 57-28-65 | 0.380 [0.306, 0.460] | **92.4 / 59.0** | 10.00 opp / **27.4 self** | 4.85 / 5.15 (abs 1.68) | **117 target / 3 TL wins / 28 draws** | **75/122 (61.5%)** |
| control (live) | mm-mcts | 100 | 42-1-57 | 0.420 [0.328, 0.518] | 57.4 / 39.0 | 6.22 | — | 65 control, 29 lockout | n/a |
| + majority_spawn_bonus | mm-mcts | 100 | 43-4-53 | 0.430 [0.337, 0.528] | **70.0 / 43.0** | 7.88 | 3.65 / 4.23 (abs 3.60) | **80 target / 6 TL wins / 4 draws** | 38/96 (39.6%) |
| capture-score base | mm-mcts | 100 | 40-12-48 | 0.400 [0.309, 0.498] | **96.4 / 58.0** | 6.42 | 2.91 / 3.51 (abs 3.62) | **57 target / 18 TL wins / 12 draws / 12 lockout** | 19/88 (21.6%) |
| + capture_cascade | mm-mcts | 100 | 50-1-49 | 0.500 [0.404, 0.596] | **37.4 / 24.5** | 5.32 | 2.67 / 2.65 (abs 5.08) | **50 target / 4 TL wins / 1 draw / 45 lockout** | 10/99 (10.1%) |

Control concentration (150 games): dest rings **C 5.4 / I 26.5 / S 52.6 / O 15.5%**. Capture nodes 7.3 / 29.8 / 44.3 / 18.6%. Inner MOVEs that actually take an opponent piece: **16.2%** (307/1893). End occupancy is inner-heavy even though actions go through spawn: remaining pieces C 11.3 / **I 55.8** / S 23.7 / O 9.2%, center held at end **86/150**. First opponent capture mean turn 8.1. Recapture-within-2: 14.1% (137/972). Piece usage: 2-ring 3847, 3-ring 3052, 1-ring 464; marks ~50/50 dot/triangle; lights normal 5806 / zag 2640 / bead 1732 / braid 1390. SWAP 78, CONVERT 99 (ratchet counterplay).

Base scoring concentration (150 games): dest **C 2.6 / I 24.6 / S 35.5 / O 37.2%** — more outer, less spawn/center than control. Capture nodes 4.0 / 25.9 / 44.3 / 25.8%. Inner MOVE capturing **5.3%** (327/6118) vs control 16.2%. End occupancy C 9.5 / I 34.0 / S 20.9 / **O 35.6%**, center held 87/150. First capture mean t7.1. Recapture-within-2 14.0% (same as control). CONVERT 0, SWAP 28, CLAIM 2. Tokens/side 1.04 vs control 2.73. P2 leads 47.9% of plies. Piece usage still 2-ring then 3-ring (14132 / 9976 / 763).

Majority-spawn concentration (150 games): dest **C 3.4 / I 25.2 / S 46.9 / O 24.4%** — spawn-heavy like control, not outer like base. Capture nodes 6.9 / 25.3 / 51.7 / 16.1%. Inner MOVE capturing **10.1%** (468/4638). End occupancy C 8.3 / I 41.6 / S 31.6 / O 18.5%, center held 88/150. Majority awards **7241** (tokens/side 13.41). SWAP 104, CONVERT 0. Comebacks **54.8%**. Score plies nearly even (P1 33% / P2 36% / tied 31%).

Capture-cascade concentration (150 games): dest C 3.1 / I 24.4 / S 39.7 / O 32.8%. Inner MOVE capturing **4.5%**. Capture-ratchets **447** (CONVERT 99, SWAP 119, extra ADD 211) — the machinery fires, but opp caps drop to 4.23 and P2 leads 61% of plies. Recapture-within-2 falls to 6.0% (the extra action lets the capturer leave contact). 42 lockouts. Solo cascade makes P2 even stronger; keep it only as a combo with majority.

Unlimited-spawn concentration (150 games): dest **C 2.7 / I 19.0 / S 57.1 / O 21.3%** — fight is occupying spawn nodes so the opponent cannot land recycled tiles. Capture nodes 2.6 / 16.3 / **67.0** / 14.2%. Inner MOVE capturing **20.7%** (highest of any arm, including control). All 5363 captures return to `owner`. End occupancy spawn-heavy (52%). 11+6 games with repeats≥3 (one max_rep=60). Fair P1 WR, 113/150 target — but the center is not the prize.

Majority+cascade combo (150 games): dest C 3.6 / I 23.1 / S 49.3 / O 24.0%. Inner MOVE capturing 6.6%. Capture-ratchets 488 (CONVERT 92, SWAP 177) plus 7695 majority awards. **Worse than majority alone**: 48 target wins vs 76, 34 lockouts, P1 WR 0.360 vs 0.387, median 131 vs 108.5. Do not ship this combo.

Majority+unlimited combo (150 games): dest C 3.9 / I 19.9 / **S 59.6** / O 16.6% — spawn-starve like unlimited, not inner-fight like majority. Inner MOVE capturing 19.2%. All 5607 captures return to `owner`. Majority awards 4598 (tokens/side 18.82). **Finishes like unlimited** (117/150 target vs 113) but **P1 WR is majority-like** (0.380 vs unlimited 0.480). Comebacks 61.5% (highest). 14 games both hit the cap and looped ≥3. SWAP 43, CONVERT 0. Score plies are tied 58.8% of the time — a closer race that still belongs to whoever occupies spawn. Do not prefer this combo over unlimited (fairer finish) or majority (less looping, more control-like).

Base scoring vs MCTS (100 games): dest **C 3.0 / I 20.4 / S 41.8 / O 34.8%**. Inner MOVE capturing 10.7% (vs 5.3% mm-mm). **57/100 target** vs 23/150 mm-mm; median **58 vs 200**. Same lopsided split as majority-vs-MCTS: minimax as P1 is **39-7-4**, MCTS as P1 is **1-5-44**. Stall is two equal exactplay bots; a weaker side gets farmed to 6 even without majority tokens. Recapture-within-2 8.3%. CONVERT 0, SWAP 0. Tokens/side 1.64. End occupancy mixed (center held 59/100). Piece usage still 2-ring then 3-ring (5505 / 3856 / 334); marks ~53/47 dot/triangle.

Capture-cascade vs MCTS (100 games): dest C 2.9 / I 21.0 / **S 55.3** / O 20.8%. Inner MOVE capturing **33.4%**. Capture-ratchets **457** (CONVERT 124, SWAP 39, extra ADD 212). Recapture-within-2 **0.6%** — the extra action leaves contact. Headline P1 WR 0.500 is a seat-swap artifact: minimax as P1 is **50-0-0**, MCTS as P1 is **0-1-49**. 45 lockouts, median 24.5. The extra action turns a skill gap into an instant snowball. Do not ship solo cascade.

## Qualitative (control 150 + smoke)

Control seed=308 (lockout, t49, P1): 2-2-1-1 ADDs on spawns, P1 sits center at **t7**, P2 answers inner 12 at t8 taking two. Later P2 SWAP inner 9 (t24), P1 recaptures center at t29 (two caps). Cascade ratchets at t45 and t48 each give P1 a CONVERT (outer 10, then spawn 8). Ends lockout — typical ratchet counterplay, not a score race.

Control seed=300 (the rare 200-turn draw): P1 occupies center at **t5** with no capture, P2 takes two off inner 9 at t8 and sits center at t10. After that the game is MOVE ping-pong (181 MOVEs, 2 center dests total, 0 SWAP/CONVERT). This is the 2% stall tail (3/150 hit the cap).

Control seed=10310 (lockout t53): P1 takes center t11, P2 recaptures center t36 (two caps), cascade at t50 → P1 CONVERT spawn 11.

Base scoring seed=302 (target win, t74, 6–3 P1): P2 opens scoring at **t4** MOVE outer 4, then t8 outer 4 again (0–2). P1 equalizes via inner 6 (t13) and outer takes, P2 sits center t40 without scoring, P1 hits 6 on **t74 MOVE inner 1**. Comeback from −2.

Base scoring seed=313 (short P2 blowout, t27, 2–6): P2 farms inner 3/6/6/6/9/16. When captures happen they happen on the inner ring; this is the 15% that actually finish on target.

Base scoring seed=300 (turn-limit 3–2 P1): score stuck 3–2 from **t88 to t200**, 183 MOVEs, ping-pong outer/spawn. Center sits at t26/t74/t89 do not score. This is the median game.

Majority-spawn seed=10300 (target, t50, 6–4 P1): P2 sits center at **t4 and t8** (majority awards fire, no capture). First score is P1 **t7 MOVE inner 21**. P1 hits 6 on **t50 MOVE inner 9** (three caps). Center sitting is token-farming, not scoring.

Majority-spawn seed=300 (turn-limit 4–3 P1): 175 MOVEs. Center at t12 (P2, majority+) and t187 (P1, no award). Score stuck 4–3 into the cap. Late majority awards still fire (t198, t200) but nobody takes the next capture. 2-ring 10841 / 3-ring 6595 / 1-ring 1154; dots 10503 vs triangle 8087 — same piece mix as control, more of them.

Majority-spawn seed=302 (target, t45, 5–6 P2): P2 wins on target after trailing −2. 76/150 games actually hit 6; 54.8% of decided games are comebacks. Tokens/side 13.4 vs 1.0 on base — extra spawn is spent, more pieces hit the board, more get captured (8.56 vs 6.00). Still 62/150 hit the cap, so the lever is real but not sufficient alone.

Majority+unlimited seed=300 (target, t99, 5–6 P2): P2 sits center at **t6** (majority award, no capture). First score is P1 t13 MOVE inner 18. Tied 5–5 at t98; P2 hits 6 on **t99 MOVE outer 22** (two caps). Spawn/outer finish, not center.

Majority+unlimited seed=304 (200-turn 5–5 draw): 105 ADDs / 95 MOVEs — recycle looping. Center sits at t2/t28/t50 with majority awards and no points. Late ply is ADD spawn 23 trading self-captures, score stuck 5–5.

Majority+unlimited seed=327 (comeback from −5, t199, 5–6 P2): P1 led 5–1 while sitting center t43. P2 equalized, sat center t172 without scoring, then **t199 MOVE inner 3** for the sixth. Largest surveyed comeback; still not a center fight.

Unlimited-spawn seed=10300 (target, t40, 6–4 P1): first score is P2 **ADD spawn 2** at t6 (landing on a recycled opponent). Center sits at t21/t25 without scoring. P1 hits 6 on **t40 MOVE inner 6** (three caps). Fight is spawn occupancy, not center.

Unlimited-spawn seed=300 (the 200-turn 5–5 draw): score tied 5–5 from **t69 to t200**. Center sits three times (t5, t19, t69) and never scores. Late ply is ADD/MOVE on spawn/outer. 1-rings stay rare (589 vs 8134 2-rings) — recycle does not change which pieces get played, it changes how often they come back.

Capture-cascade seed=302 (lockout t49, 0–3 P2): P2's first capture at **t4 MOVE outer 4** seats a capture-ratchet; P1 sits center t47 without scoring; P2 CONVERT spawn 5 at t49 after a two-cap inner 18. Extra action is the counterplay, and it belongs to whoever captured.

Capture-cascade seed=300 (200-turn 2–2 draw): one capture-ratchet (t41 extra ADD spawn 17) then 183 MOVEs. Center sit at t11 does not score. The extra action is not enough to force a finish when both refuse contact.

Base vs MCTS seed=10300 (minimax P1 farms MCTS, t48, 6–1): first cap t11 MOVE outer 22. P1 sits center t13 without scoring; P2 sits center t24/t42/t48 also without scoring — **center is a parking spot**. P1 hits 6 on **t48 MOVE inner 21** (two caps) while P2 is sitting origin.

Base vs MCTS seed=10314 (shortest farm, t18, 6–0): no center sits. P1 scores on inner 12/18/18. Nineteen actions. MCTS as P2 never captures.

Base vs MCTS seed=20309 (200-turn 4–4 draw, MCTS as P1): 178 MOVEs, two center sits (t21, t44), score stuck 4–4 from t44. Same ping-pong as mm-mm stalls; it is the 12% draw tail, not the median (58).

Cascade vs MCTS seed=10300 (minimax P1 lockout, t24, 5–0): first capture-ratchet **t9 MOVE inner 16**; CONVERT outer 24 at t15 and spawn 23 at t18; ends lockout on a two-cap outer 19 plus extra ADD. No center sits. Minimax swept this pairing 50-0-0.

Cascade vs MCTS seed=20300 (MCTS P1 lockout, t28, 0–4): P2's first capture at **t4 MOVE inner 6** seats the ratchet; CONVERT spawn 11 at t10. Center ping-pong t15–t25 never scores. MCTS as P1 went 0-1-49.

Cascade vs MCTS seed=20302 (score, t20, 0–6): five ratchets, one SWAP spawn 14 at t11, no center. Extra actions farm the inner ring to 6 in 20 turns.

Smoke notes below first picked this lever.

Base scoring with weight 0 does not hunt captures. Smoke_base game seed=1: opening ADDs on outer spawns (t1–t6), first capture at **t23** (P1 MOVE to node 1, takes P2 off center). End is **t200 TurnLimit, 5–4**. Late ply is MOVE ping-pong on the inner ring (t196–t200, score stuck 5–4). Dest mix: 559 outer / 217 inner / 24 center over 800 actions. Zero CLAIM/SWAP/CONVERT.

`eval_capture_score=400` made this worse (fewer opponent captures, more inner parking). See the symmetric-risk note above. Do not bake a large score weight from this n=4.

`majority_spawn_bonus` is the first lever that actually finishes games (all four hit target 6 around turn 46–88). Mean tokens/side 7.30 vs 0.95 on base — the extra spawn token is spent, so more pieces hit the board and get captured. Comebacks 2/4 (P1 seed=1 trailed −1 then won 6–5 at t88; P1 seed=10001 trailed −1 then won 6–1 at t46).

`capture_cascade` does fire (13 capture-ratchets, 3 CONVERT / 2 SWAP / 8 extra ADD) but two of four games still hit the cap.

`unlimited_spawn` returns every capture to the owner; self-captures explode (58/game) and one game had max_repeat=51 (looping).

## Honest take

Control 150-game mirror is a **fair, short game** (P1 WR 0.500, mean 49 turns). Bare capture-score vs an equal exactplay mirror is not (P1 WR 0.267, median 200), but vs MCTS it **does** finish (57/100 target, median 58) — the stall is two equal bots refusing contact, not a missing win condition. Two scoring levers finish equal-bot games: **majority-spawn** (76/150 target vs mm-mm; **80/100 vs MCTS**; more control-like rings) and **unlimited-spawn** (113/150 target, P1 WR 0.480; spawn-starve, some loops). Majority+unlimited finishes like unlimited (117 target) but keeps majority's P2 edge (P1 WR 0.380) and the spawn fight — not a better mix. Majority / base / cascade vs MCTS are all lopsided (minimax P1 42-2-6 / 39-7-4 / **50-0-0**; MCTS P1 1-2-47 / 1-5-44 / **0-1-49**). Cascade's fair-looking 0.500 P1 WR is the seat swap. Center sits rarely score under capture-score; the inner ring is where points happen. Remaining cells: inner/center trigger, target 4/8. Control still has the cleanest center fight. `eval_capture_score=400` stays off.
