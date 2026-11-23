#!/usr/bin/env python3
"""Survey capture-score (and control-baseline) JSONL.gz logs.

Reports P1 win rate, draws, length, captures, score margins, how games end
(target vs turn limit), comebacks, ring concentration (center / inner /
spawn-middle / outer), capture locations, piece-type usage, and recapture
timing (for the eval_capture_score timidness check).
"""

from __future__ import annotations

import gzip
import json
import math
import statistics
import sys
from collections import Counter
from pathlib import Path

CENTER = 0
INNER = {9, 1, 12, 3, 16, 6, 18, 21}
SPAWN = {2, 5, 8, 11, 14, 17, 20, 23}
CONTROL = INNER | {CENTER}


def load_games(path: Path) -> list[dict]:
    opener = gzip.open if path.suffix == ".gz" or path.name.endswith(".jsonl.gz") else open
    games: list[dict] = []
    with opener(path, "rt", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                games.append(json.loads(line))
    return games


def wilson_ci(k: int, n: int, z: float = 1.96) -> tuple[float, float, float]:
    if n <= 0:
        return 0.0, 0.0, 0.0
    p = k / n
    z2 = z * z
    den = 1.0 + z2 / n
    centre = (p + z2 / (2.0 * n)) / den
    margin = z * math.sqrt((p * (1.0 - p) + z2 / (4.0 * n)) / n) / den
    return p, max(0.0, centre - margin), min(1.0, centre + margin)


def node_ring(node) -> str:
    if node is None:
        return "none"
    n = int(node)
    if n == CENTER:
        return "center"
    if n in INNER:
        return "inner"
    if n in SPAWN:
        return "spawn"
    return "outer"


def dest_of(act: dict) -> int | None:
    to = act.get("to")
    if to is not None:
        return int(to)
    return None


def pct(part: int, whole: int) -> str:
    if whole <= 0:
        return "0%"
    return f"{100.0 * part / whole:.1f}%"


def survey(games: list[dict]) -> None:
    n = len(games)
    if n == 0:
        print("no games")
        return
    p1_w = p2_w = draws = 0
    reasons: Counter[str] = Counter()
    turns: list[int] = []
    caps: list[int] = []
    self_caps: list[int] = []
    p1_scores: list[int] = []
    p2_scores: list[int] = []
    margins: list[int] = []
    kinds: Counter[str] = Counter()
    dest_rings: Counter[str] = Counter()
    cap_rings: Counter[str] = Counter()
    capture_dests: Counter[str] = Counter()
    piece_weights: Counter[int] = Counter()
    piece_marks: Counter[str] = Counter()
    piece_lights: Counter[str] = Counter()
    comebacks = 0
    majority_awards = 0
    ratchet_captures = 0
    target_wins = 0
    turn_limit_wins = 0
    turn_limit_draws = 0
    end_occupied: list[int] = []
    recapture_2 = 0
    opp_cap_events = 0
    lead_plies = 0
    tied_plies = 0
    trail_plies = 0
    examples: list[str] = []
    inner_move_cap = 0
    inner_move_nocap = 0
    first_opp: list[int] = []
    center_dest_plies = 0
    end_rings: Counter[str] = Counter()
    end_center_occupied = 0

    for i, g in enumerate(games):
        end = g.get("end") or {}
        status = end.get("status")
        reason = end.get("reason") or "?"
        winner = end.get("winner")
        reasons[f"{status}:{reason}"] += 1
        t = int(g.get("turns") or 0)
        turns.append(t)
        p1s = int(end.get("p1_score") or 0)
        p2s = int(end.get("p2_score") or 0)
        p1_scores.append(p1s)
        p2_scores.append(p2s)
        margins.append(abs(p1s - p2s))
        if status == "won" and winner == 1:
            p1_w += 1
            if reason == "score":
                target_wins += 1
            elif reason == "turn_limit":
                turn_limit_wins += 1
        elif status == "won" and winner == 2:
            p2_w += 1
            if reason == "score":
                target_wins += 1
            elif reason == "turn_limit":
                turn_limit_wins += 1
        else:
            draws += 1
            if reason == "turn_limit":
                turn_limit_draws += 1

        actions = g.get("actions") or []
        opp_caps = 0
        own_caps = 0
        min_lead: dict[int, int] = {1: 0, 2: 0}
        last_opp_cap_by: dict[int, int] = {}
        first_cap_turn: int | None = None
        for ai, act in enumerate(actions):
            kind = act.get("kind") or "?"
            kinds[kind] += 1
            dest = dest_of(act)
            dest_rings[node_ring(dest)] += 1
            if node_ring(dest) == "center":
                center_dest_plies += 1
            piece = act.get("piece") or {}
            w = piece.get("weight") or piece.get("n_rings")
            if w is not None:
                piece_weights[int(w)] += 1
            mark = piece.get("capture") or "triangle"
            piece_marks[str(mark)] += 1
            n_rings = int(piece.get("n_rings") or 0)
            normal = int(piece.get("normal_rings") or 0)
            zag = int(piece.get("zag_rings") or 0)
            bead = int(piece.get("bead_rings") or 0)
            braid = max(0, n_rings - (normal + zag + bead))
            if braid:
                piece_lights["braid"] += 1
            if zag:
                piece_lights["zag"] += 1
            if bead:
                piece_lights["bead"] += 1
            if normal:
                piece_lights["normal"] += 1
            if act.get("majority_spawn"):
                majority_awards += 1
            if act.get("ratchet_kind") == "capture":
                ratchet_captures += 1
            p1_now = int(act.get("p1_score") or 0)
            p2_now = int(act.get("p2_score") or 0)
            min_lead[1] = min(min_lead[1], p1_now - p2_now)
            min_lead[2] = min(min_lead[2], p2_now - p1_now)
            if p1_now > p2_now:
                lead_plies += 1
            elif p2_now > p1_now:
                trail_plies += 1
            else:
                tied_plies += 1
            player = act.get("player")
            opp_this = 0
            for cap in act.get("captures") or []:
                capture_dests[str(cap.get("dest") or "?")] += 1
                cap_rings[node_ring(cap.get("node"))] += 1
                owner = cap.get("owner")
                if owner is not None and player is not None and int(owner) != int(player):
                    opp_caps += 1
                    opp_this += 1
                    opp_cap_events += 1
                    last_opp_cap_by[int(player)] = ai
                    other = 1 if int(player) == 2 else 2
                    prev = last_opp_cap_by.get(other)
                    if prev is not None and ai - prev <= 2:
                        recapture_2 += 1
                    if first_cap_turn is None:
                        first_cap_turn = int(act.get("turn") or 0)
                else:
                    own_caps += 1
            if kind == "MOVE" and node_ring(dest) == "inner":
                if opp_this:
                    inner_move_cap += 1
                else:
                    inner_move_nocap += 1
        caps.append(opp_caps)
        self_caps.append(own_caps)
        if first_cap_turn is not None:
            first_opp.append(first_cap_turn)
        if status == "won" and winner in (1, 2) and min_lead[int(winner)] < 0:
            comebacks += 1
            if len(examples) < 8:
                examples.append(
                    f"game {i} seed={g.get('seed')} winner=P{winner} "
                    f"turns={t} score={p1s}-{p2s} reason={reason} "
                    f"max_deficit={min_lead[int(winner)]}"
                )
        final = g.get("final_state") or {}
        occ = 0
        center_held = False
        for key in ("p1_board", "p2_board"):
            for piece in final.get(key) or []:
                occ += 1
                r = node_ring(piece.get("node"))
                end_rings[r] += 1
                if r == "center":
                    center_held = True
        end_occupied.append(occ)
        if center_held:
            end_center_occupied += 1

    p, lo, hi = wilson_ci(p1_w, n)
    action_n = sum(dest_rings.values())
    cap_n = sum(cap_rings.values())
    print(f"n={n}  P1 {p1_w}-{draws}-{p2_w}  P1_wr={p:.3f} [{lo:.3f},{hi:.3f}]")
    print(f"end_reasons  {dict(reasons)}")
    print(
        f"ends  target_wins={target_wins} turn_limit_wins={turn_limit_wins} "
        f"turn_limit_draws={turn_limit_draws}"
    )
    print(
        f"length  mean={statistics.mean(turns):.2f} median={statistics.median(turns):.1f} "
        f"min={min(turns)} max={max(turns)}"
    )
    print(
        f"captures/game  opp mean={statistics.mean(caps):.2f} median={statistics.median(caps):.1f}  "
        f"self mean={statistics.mean(self_caps):.2f}"
    )
    print(
        f"score  mean_p1={statistics.mean(p1_scores):.2f} mean_p2={statistics.mean(p2_scores):.2f} "
        f"mean_abs_margin={statistics.mean(margins):.2f} median_abs_margin={statistics.median(margins):.1f}"
    )
    decided = p1_w + p2_w
    print(
        f"comebacks (winner trailed on capture-score) {comebacks}/{decided} "
        f"({0 if decided == 0 else 100 * comebacks / decided:.1f}%)"
    )
    print(f"majority_spawn awards (actions)={majority_awards}  capture-ratchets={ratchet_captures}")
    print(f"actions  {dict(kinds)}")
    print(
        "dest rings  "
        + "  ".join(
            f"{k}={dest_rings[k]} ({pct(dest_rings[k], action_n)})"
            for k in ("center", "inner", "spawn", "outer", "none")
            if dest_rings[k]
        )
    )
    inner_moves = inner_move_cap + inner_move_nocap
    print(
        f"inner MOVE capturing opp  {inner_move_cap}/{inner_moves} "
        f"({pct(inner_move_cap, inner_moves)}); parking={inner_move_nocap}"
    )
    print(f"center dest plies  {center_dest_plies} ({pct(center_dest_plies, action_n)})")
    if first_opp:
        print(
            f"first opp-capture turn  mean={statistics.mean(first_opp):.1f} "
            f"median={statistics.median(first_opp):.1f} n={len(first_opp)}/{n}"
        )
    print(
        "capture nodes  "
        + "  ".join(
            f"{k}={cap_rings[k]} ({pct(cap_rings[k], cap_n)})"
            for k in ("center", "inner", "spawn", "outer")
            if cap_rings[k]
        )
    )
    print(f"capture dests  {dict(capture_dests)}")
    print(f"piece n_rings  {dict(sorted(piece_weights.items()))}")
    print(f"piece marks  {dict(piece_marks)}")
    print(f"piece lights  {dict(piece_lights)}")
    ply = lead_plies + trail_plies + tied_plies
    print(
        f"score plies  p1_lead={lead_plies} ({pct(lead_plies, ply)})  "
        f"p2_lead={trail_plies} ({pct(trail_plies, ply)})  "
        f"tied={tied_plies} ({pct(tied_plies, ply)})"
    )
    print(
        f"recapture within 2 plies  {recapture_2}/{opp_cap_events} "
        f"({pct(recapture_2, opp_cap_events)})"
    )
    if end_occupied:
        end_n = sum(end_rings.values())
        print(
            f"end occupancy  mean={statistics.mean(end_occupied):.2f} "
            f"median={statistics.median(end_occupied):.1f}  "
            + "  ".join(
                f"{k}={end_rings[k]} ({pct(end_rings[k], end_n)})"
                for k in ("center", "inner", "spawn", "outer")
                if end_rings[k]
            )
            + f"  center_held={end_center_occupied}/{n}"
        )
    if examples:
        print("comeback examples:")
        for e in examples:
            print(f"  {e}")


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: survey.py <file.jsonl.gz> [...]", file=sys.stderr)
        return 2
    for arg in argv[1:]:
        path = Path(arg)
        print(f"\n# {path}")
        survey(load_games(path))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
