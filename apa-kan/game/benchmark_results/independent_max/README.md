# Independent-max vs additive (light H2H)

Compares the experimental default (`--channel-mode independent-max`) with
RFC 002 additive stacking (`--channel-mode additive`). Doubles allowed,
zags on, `zag_mismatch=sum`. Do not overwrite; quote numbers from the
committed `*.txt` logs in this directory. n=20 per ruleset is a **light**
sample (2 games × 2 seats × 5 matchups), not the 108-game package.

The two logs also differ in **generation**: independent-max elects light
slots 5:3:1 (singles allowed); additive still rolls zag 1/7 then 2–3 bead
groups. Scoring and bag mix move together.

## Package

Release `apa-kan-bench`, `--budget nodes:132000,iters:2600`, `--pick search`,
`--jobs 4`, `--no-depth-probe`, `--no-same-piece-twice off`, `--zags on`,
`--pairings minimax:mcts,mcts:minimax`, `--deck-matchups
mixed:mixed,braid:braid,bead:bead,braid:bead,bead:braid`, `--games 2`.

| File | Rule | Games | Wall-clock |
|---|---|---|---|
| [`h2h.txt`](h2h.txt) | independent-max (live default) | 20 | 19:43:36Z–19:59:32Z |
| [`h2h_additive.txt`](h2h_additive.txt) | additive (RFC 002) | 20 | 19:59:32Z–20:06:12Z |

Per-game JSONL: [`h2h.jsonl.gz`](h2h.jsonl.gz), [`h2h_additive.jsonl.gz`](h2h_additive.jsonl.gz).

## Summary (quoted)

From [`h2h.txt`](h2h.txt):

```
# rules: ... no_same_piece_twice=off ... zag_numer=1 ... channel_mode=independent-max ... zag_mismatch=sum ...
# ring_process_zags  rings=25796 zags=6742 rate=0.2614
# ring_process_braids  pieces=20000 braided=2815 piece_rate=0.1407 rings=25796 braid_rings=5631 ring_rate=0.2183
# ring_process_beads  pieces=20000 grouped=35 piece_rate=0.0018 rings=25796 beads=2259 ring_rate=0.0876
# seat_totals  P1 14-0-6  P2 6-0-14  games=20
# end_reasons  elim=0 locked_out=4 center_hold=0 petal_maj=0 control=7 insuff=0 low_value=9 low_value_wins=9 low_value_draws=0 turn_limit=0 double_pass=0
# captures  opponent_tiles_removed=103  avg_per_game=5.15  self_captures=131  self_avg_per_game=6.55
# bead_captures  contrib=107 needed=46 spawn_two_ring_single_bead=1  of_total=103
# matchup_totals  mixed vs mixed 3-0-1 avg_turns=51.25  |  braid vs braid 3-0-1 avg_turns=70.75  |  bead vs bead 3-0-1 avg_turns=33.75  |  braid vs bead 1-0-3 avg_turns=49.25  |  bead vs braid 4-0-0 avg_turns=66.50
# archetype_seat  mixed P1=3-0-1 P2=1-0-3  |  braid P1=4-0-4 P2=1-0-7  |  bead P1=7-0-1 P2=4-0-4
```

Unweighted mean of the ten pairing `avg_turns` columns: **54.30**.

From [`h2h_additive.txt`](h2h_additive.txt):

```
# rules: ... no_same_piece_twice=off ... zag_numer=1 ... channel_mode=additive ... zag_mismatch=sum ...
# ring_process_zags  rings=29257 zags=2858 rate=0.0977
# ring_process_braids  pieces=20000 braided=2783 piece_rate=0.1391 rings=29257 braid_rings=5568 ring_rate=0.1903
# ring_process_beads  pieces=20000 grouped=2448 piece_rate=0.1224 rings=29257 beads=5901 ring_rate=0.2017
# seat_totals  P1 6-2-12  P2 12-2-6  games=20
# end_reasons  elim=1 locked_out=4 center_hold=0 petal_maj=0 control=3 insuff=0 low_value=11 low_value_wins=10 low_value_draws=1 turn_limit=1 double_pass=0
# captures  opponent_tiles_removed=137  avg_per_game=6.85  self_captures=127  self_avg_per_game=6.35
# bead_captures  contrib=94 needed=67 spawn_two_ring_single_bead=0  of_total=137
# matchup_totals  mixed vs mixed 2-0-2 avg_turns=32.50  |  braid vs braid 1-1-2 avg_turns=26.75  |  bead vs bead 1-0-3 avg_turns=36.75  |  braid vs bead 0-1-3 avg_turns=74.00  |  bead vs braid 2-0-2 avg_turns=33.00
# archetype_seat  mixed P1=2-0-2 P2=2-0-2  |  braid P1=1-2-5 P2=4-1-3  |  bead P1=3-0-5 P2=6-1-1
```

Unweighted mean of the ten pairing `avg_turns` columns: **40.60**.

| Log | P1 W-D-L | Mean avg turns | Ends (lock/ctrl/LV/TL/elim) | Opp / self /game |
|---|---|---|---|---|
| `h2h` independent-max | **14-0-6** | **54.30** | 4 / 7 / 9 / 0 / 0 | 5.15 (103) / 6.55 (131) |
| `h2h_additive` | **6-2-12** | **40.60** | 4 / 3 / 11 / 1 / 1 | 6.85 (137) / 6.35 (127) |

## Notes (n=20, not a lock)

- First-player rate flipped: independent-max P1 **14-0-6**, additive P1 **6-2-12**. Too few games to treat as a seat-balance result.
- Independent-max `bead vs braid` is **4-0-0** and `braid vs bead` is **1-0-3** (bead wins 7 of 8 cross games). Additive cross is **0-1-3** + **2-0-2**. Light sample; collectors also change under 5:3:1 stuffing.
- Generation: independent-max zag ring rate **0.2614** vs additive **0.0977**; bead ring rate **0.0876** vs **0.2017**. One independent-max capture is `spawn_two_ring_single_bead=1`; additive has **0** (no singles).
- `zag_mismatch=sum` on both logs. Independent-max still takes the **max** of that zag component against the other channels.
