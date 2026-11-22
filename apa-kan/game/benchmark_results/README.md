# Search H2H logs

Live ruleset: ratchet reward (flips anywhere after the second ratchet) + 2-2-1-1 opening on the #83 scoring package (additive, braid-only-exact, double-counted center↔inner, 4:3:1, drift 160, same-deck 27, adjacent normal-only swap with no swap-back). Doubles allowed (`--no-same-piece-twice off`) is the locked default.

The source experiment for that lock-in is [`opening_2211/`](opening_2211/) (exactplay minimax mirror, light budget, zags on, `--zag-mismatch sum`): **2211 r1** P1 **152-6-142** (50.7%), mean / median turns **49.36 / 41.5**.

Comparison toggles kept on the CLI: `--cascade-winner-swap off`, `--opening 1-1-1-1`, and `--cascade-turn-gain` when the ratchet is off. Retired experiment flags (`--extra-convert-mode`, `--extra-swap-mode`, `--center-mismatch`, `--channel-mode`, `--braid-scoring`, `--bead-scoring`, `--link-once`, center-hold / petal-majority / turn-count wins, …) error closed.

Historical logs and scripts: [`archive/`](archive/). JSONL stays in the original directories so replay tests keep pinning those paths. Do not overwrite committed logs.

`--budget nodes:132000,iters:2600` is the default full-budget H2H (rounded mean of a single-threaded 1000 ms/move calibrate). Light cells in `opening_2211/` and `cascade_winner_swap/` used `nodes:33000,iters:650`. `--jobs` defaults to `available_parallelism`; each search stays single-threaded; JSONL is written in pairing-then-game order.
