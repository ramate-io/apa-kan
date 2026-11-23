# Archived rule experiments

JSONL and H2H logs stay in their original `benchmark_results/` directories so replay tests keep pinning those paths. This note is the index of experiments whose **rule variants are no longer live CLI** (the engine still reconstructs the logs).

The locked live package is ratchet reward (flips anywhere after the second ratchet) + 2-2-1-1 opening on the #83 scoring defaults. Teachable rules: [root README How to play](../../../../README.md). Comparison toggles: `--cascade-winner-swap off`, `--opening 1-1-1-1`.

| Directory | What it compared | Why it is archived |
|---|---|---|
| [`../opening_2211/`](../opening_2211/) | 2-2-1-1 on/off, with and without the ratchet | **Now the live default.** Logs remain the source experiment (P1 ~50%, mean ~49 on 2211 r1). |
| [`../cascade_winner_swap/`](../cascade_winner_swap/) | Next-move ratchet vs extra-turn; convert/swap filters; center +1 mismatch | Ratchet is live. `--extra-convert-mode`, `--extra-swap-mode`, `--center-mismatch`, `--extra-turn-action` were removed (flips are always any opponent piece). |
| [`../control_cascade_turn_gain/`](../control_cascade_turn_gain/) | Extra turn on a non-winning cascade drop | Superseded by the ratchet. `--cascade-turn-gain` remains only for off-ratchet comparison. |
| [`../center_link_once/`](../center_link_once/) | Unique-neighbor vs double-counted center↔inner; braid-only-match | `--link-once` and `--braid-scoring` CLI removed. Live scoring is per-arc / braid-only-exact. |
| [`../single_braid_exact_swap/`](../single_braid_exact_swap/) | Exact braids, single braids, swap, link-once | Exact braids + single braids + normal-only swap are live; link-once CLI is gone. |
| [`../independent_max/`](../independent_max/) | Independent-max vs additive channels | `--channel-mode` CLI removed. Live is additive. |
| [`../bead_shared_channel/`](../bead_shared_channel/) | Independent vs shared bead scoring | `--bead-scoring` CLI removed. Live is independent (beads separate). |
| [`../ring_dist_channels/`](../ring_dist_channels/) | 4:3:1 vs 7:5:4 × additive vs independent-max | Ring weights 4:3:1 are live; channel-mode CLI is gone. |
| [`../control_cascade/`](../control_cascade/) | Control-cascade endgame on independent-max | Endgame is live; the channel default is not. |
| [`../local_claim/`](../local_claim/) | Local-claim vs cascade captures | Local-claim is live. `--capture-mode cascade` remains. |
| [`../singles_swap/`](../singles_swap/) | Swap initiator / echo variants | Live is adjacent normal-only, no swap-back. `--singles-swap` remains. |
| [`../exactplay/`](../exactplay/) | Search-profile hillclimb | Not a rules experiment. Profile still loads. |
| Older H2H (`zag_color_exclusion/`, `capture_modifiers/`, `work_budget_*`, `center_double_edge_*`, bag-11 / pass-through / zag-mismatch `.txt` logs) | Pre-#83 packages | Kept for replay; do not overwrite. |

Re-running an archived script against current binaries needs the comparison flags pinned (for example `--opening-double off` on the pre-2211 ratchet cell, `--cascade-winner-swap off` on a no-ratchet baseline). Removed flags error closed.
