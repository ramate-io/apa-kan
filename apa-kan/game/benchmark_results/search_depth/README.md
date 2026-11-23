# Search depth diagnosis

Measured on the locked ruleset (ratchet reward + 2-2-1-1), exactplay
minimax, zags on, doubles allowed, `--zag-mismatch sum`. Positions come
from [`../opening_2211/h2h_n300_mm_r1.jsonl.gz`](../opening_2211/h2h_n300_mm_r1.jsonl.gz)
(300 games, light budget). Reproduce:

```bash
cargo run -p apa-kan --bin apa-kan-analyze --release --no-default-features -- \
  --diagnose --sample 3 --out apa-kan/game/benchmark_results/search_depth/diagnose.txt
```

Raw rows: [`diagnose.txt`](diagnose.txt). 16×/64× control-win timings
below were captured in the same binary on the same positions.

## Branching (legal actions)

`search=` is [`legal_search_moves`](../../src/ai/search_move.rs) (ADD
collapsed by tile kind). 14 847 positions across 300 games.

| Phase | n | all avg/max | search avg/max | add | move | swap | claim | convert |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 14847 | **47.4 / 116** | 45.9 / 116 | 28.5 / 96 | 18.6 / 64 | 0.3 / 50 | 0.0 / 1 | 0.0 / 6 |
| early (t≤8) | 2700 | **65.2 / 100** | 63.8 | 54.5 | 10.7 | 0.0 | 0 | 0 |
| mid (t≤35) | 7474 | 47.5 / 116 | 45.7 | 27.7 | 19.5 | 0.3 | 0.0 | 0.0 |
| late | 4673 | 37.0 / 104 | 35.8 | 14.7 | 21.8 | 0.5 | 0.0 | 0.1 |
| ratchet reward | 549 | **54.4 / 116** | 51.3 | 30.6 | 15.4 | **7.5 / 50** | 0.0 | **0.9 / 6** |

Kind-collapsing ADD saves ~1.5 moves on average (47.4 → 45.9), not a
factor of two. SWAP/CONVERT stay rare except on ratchet turns (swap
jumps from 0.3 to 7.5; convert 0.0 to 0.9). Terminals in this log:
**control 201**, lockout 75, elimination 8 — self-play at light budget
*does* convert control when it sees it.

## Depth actually reached

Light work budget `nodes:33000` (~250 ms at 130 kN/s). Playable app
default is `--think-ms 80`.

| Position | legal | light depth / nps / ms | `--think-ms 80` depth / nodes |
|---|---:|---|---|
| opening (g1 a1) | 88 | **2** / 145 kN/s / 228 | **2** / 22 k |
| opening 2nd ply | 77 | **2** / 239 kN/s / 138 | — |
| mid t9–11 | 49–66 | **2–3** / 55–84 kN/s / 391–598 | **2** / 4.9 k |
| late t36–38 | 9–66 | **3–4** / 51 kN/s / 628–651 | **2** / 3.3 k |

Quiescence eats ~30–40% of nodes (`q_nodes` ≈ 10–14 k of 33 k). LMR
almost never fires: `lmr_min_depth=3` and ID only completes depth 2–4.
PVS null-window searches are the main interior work (`pvs_null` thousands,
`pvs_re` a dozen). First-move beta cutoffs are **44–97%** (96% on the
opening) — ordering is already good; TT cuts 0–30% of probes.

**2-2-1-1 and ratchet vs ply.** Each action costs one ID ply, including
same-side extras. At opening depth 2 the tree is: P1 action, leftover
opening action, then quiescence — **P2 is never fully searched**.
`same_side=2773` / `opening=2773` on the first ply. Midgame singles have
`same_side=0`. Ratchet extras reappear as same-side children (e.g. 374
on a control-win leaf).

## Terminal detection

Control / lockout / claim / ratchet-reward wins **are** `WIN±ply` inside
search (`terminal_score` on `game.status()` after apply, including
immediate unique-holder drops and hold completions). Not a detection
bug.

Evidence: one ply before a hold win, min_depth=1 in **75–426 nodes /
1–5 ms**, `first_win_ply=1`, score 49950. Late-game samples also record
`term_w` at `win_ply` 4–5 when the line is deeper.

Quiescence is capture-only. A quiet occupy that only *starts* a hold is
not a terminal; it needs the opponent reply and another own action
(depth 3). That is a horizon, not a missing `status()` check.

## Forced control wins (hold)

Four control-ended games, position *N* actions before the last move.
`min_depth` is the smallest completed ID with score > `WIN/2` (STM).

| Game | kind | back | min depth / nodes / ms | light (33 k) | 4× (132 k) | 16× (528 k) | 64× (2.1 M) |
|---|---|---:|---|---|---|---|---|
| g1 | hold | 1 | **1** / 426 / 5 ms | d4, 175 ms | d6, 0.9 s | d7, 3.7 s | d8, 14.7 s |
| g1 | hold | 3 | STM is losing (`-WIN`); win_ply=3 | d5 | d7 | — | — |
| g2 a27 | hold | 1 | **1** / 262 / 1 ms | d5, 142 ms | d7, 0.6 s | d8, 2.3 s | d10, 10.3 s |
| g2 a27 | hold | 3 | **1** / 472 / 6 ms | d3, 250 ms | d4, 0.9 s | — | — |
| g2 a39 | hold | 3 | **3** / 3029 / 26 ms | d5, 227 ms | d6, 1.3 s | d8, 5.5 s (b1) | d9, 20.2 s (b1) |
| g3 a38 | hold | 3 | **3** / 29507 / 1.1 s | d3, 1.1 s | d4, 2.9 s | d8, 5.7 s (b1) | d10, 24.1 s (b1) |
| g1 | hold | 6 | **7** / 1.3 M / 16 s | not at 33 k | — | — | — |

1-ply takes are free. **3-ply hold wins need depth 3** (~3 k–30 k nodes
when the line is forced; more when it is not). Light budget *sometimes*
completes depth 3 in midgame and usually in late game. The playable
**80 ms default never completed depth 3** on the early/mid/late sample
(depth 2, 3–22 k nodes). That is the gap Liam is seeing: occupy-then-hold
is invisible at the GUI budget.

`back=2` is often `min_depth=none` because it is the **victim's** turn:
there is no STM win, only a loss if they fail to disrupt. Search reports
that correctly (`-WIN` at back=3 for g1).

## Bottleneck (ranked)

1. **Ply budget vs 3-ply holds.** ID depth 2–3 at live budgets. Opening
   extras spend those plies on same-side actions. Not a missing terminal.
2. **Raw move space.** ~65 legal early (mostly ADD), ~47 mid, ~37 late.
   Kind-collapse barely helps. Ratchet SWAP blows up (max 50).
3. **Quiescence share.** 30–40% of nodes on captures that are not the
   control line.
4. **LMR idle.** `lmr_min_depth=3` while completed depth is 2–4, so LMR
   almost never reduces.
5. **Eval horizon.** Occupying the control set is a geometry bonus, not
   mate, until the hold completes. At depth 2 the bot can prefer a
   capture.

NPS is 50–240 k depending on phase (slower late: more eval/contact).
Move generation is not the limiter; node count × branching is.

## Ranked fixes (not implemented here unless noted)

| Rank | Fix | Expected depth | Cost | Notes |
|---|---|---|---|---|
| 1 | Raise playable `--think-ms` (80 → ~400) **or** use the light work budget in the GUI | +1 ply (depth 3 in mid/late) | Tiny | Enough to see 3-ply holds. Documented workaround: `--bot2 minimax --think-ms 400`. |
| 2 | Same-side extra-move extensions (opening leftover / ratchet reward charge 0 or ½ ply) | +1 ply on opening and ratchet lines | Small | General, not a tactic. Opening depth 2 would then include a P2 reply. |
| 3 | Iterative deepening on a time/work budget with aspiration (already ID; add aspiration + fail-soft) | Better PV stability, slight NPS | Small | Ordering is already 50–96% first-cut. |
| 4 | Mate / proof-number / win-extension: if a child is `WIN-ε`, search it extra ply | Finds 3-ply holds at depth 2 | Medium | General terminal extension, not control-specific. |
| 5 | Quiet control-occupying moves in quiescence (or qsearch any move that raises `control_count`) | Hides fewer occupy-then-hold leaves | Small–medium | Slightly opinionated (geometry in qsearch). |
| 6 | Faster movegen / incremental eval | NPS ×1.5–3 → +0.5 ply | Medium | Eval+apply dominate (NPS drops late). |
| 7 | Symmetry / duplicate ADD beyond kind-collapse; board rotation | ~5–15% less branching | Medium | 8-fold rosette symmetry is real but hashing must match. |
| 8 | Proof-number search at the root when eval is near mate | Solves forced wins | Large | Separate solver, keep minimax for the rest. |

No terminal-detection or depth-accounting *bug* was found. Search
behavior of `--search-profile exactplay` is unchanged; this PR only
counts. The playable default is now that profile; use `--think-ms 400`
(or higher) if the bot should actually *see* hold wins.
