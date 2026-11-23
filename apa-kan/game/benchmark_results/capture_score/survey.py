#!/usr/bin/env python3
"""Survey capture-score (and control-baseline) JSONL.gz logs.

Reports P1 win rate, draws, length, captures, score margins, how games end
(target vs turn limit), comebacks, action mix, and dest-ring concentration.
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
    if node == CENTER:
        return "center"
    if node in INNER:
        return "inner"
    return "outer"


def dest_of(act: dict) -> int | None:
    to = act.get("to")
    if to is not None:
        return int(to)
    return None


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
    capture_dests: Counter[str] = Counter()
    piece_weights: Counter[int] = Counter()
    comebacks = 0
    trail_then_win = 0
    majority_awards = 0
    ratchet_captures = 0
    target_wins = 0
    turn_limit_wins = 0
    turn_limit_draws = 0
    end_occupied: list[int] = []
    examples: list[str] = []

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
        for act in actions:
            kind = act.get("kind") or "?"
            kinds[kind] += 1
            ring = node_ring(dest_of(act))
            dest_rings[ring] += 1
            piece = act.get("piece") or {}
            w = piece.get("weight")
            if w is not None:
                piece_weights[int(w)] += 1
            if act.get("majority_spawn"):
                majority_awards += 1
            if act.get("ratchet_kind") == "capture":
                ratchet_captures += 1
            p1_now = int(act.get("p1_score") or 0)
            p2_now = int(act.get("p2_score") or 0)
            min_lead[1] = min(min_lead[1], p1_now - p2_now)
            min_lead[2] = min(min_lead[2], p2_now - p1_now)
            player = act.get("player")
            for cap in act.get("captures") or []:
                capture_dests[str(cap.get("dest") or "?")] += 1
                owner = cap.get("owner")
                if owner is not None and player is not None and int(owner) != int(player):
                    opp_caps += 1
                else:
                    own_caps += 1
        caps.append(opp_caps)
        self_caps.append(own_caps)
        if status == "won" and winner in (1, 2) and min_lead[int(winner)] < 0:
            comebacks += 1
            trail_then_win += 1
            if len(examples) < 8:
                examples.append(
                    f"game {i} seed={g.get('seed')} winner=P{winner} "
                    f"turns={t} score={p1s}-{p2s} reason={reason} "
                    f"max_deficit={min_lead[int(winner)]}"
                )
        final = g.get("final_state") or {}
        occ = 0
        for key in ("p1_board", "p2_board"):
            occ += len(final.get(key) or [])
        if not occ:
            # Reconstruct occupancy from last action if final_state omitted.
            occ = -1
        end_occupied.append(max(occ, 0))

    p, lo, hi = wilson_ci(p1_w, n)
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
    print(
        f"comebacks (winner trailed on capture-score) {comebacks}/{p1_w + p2_w} "
        f"({0 if (p1_w + p2_w) == 0 else 100 * comebacks / (p1_w + p2_w):.1f}%)"
    )
    print(f"majority_spawn awards (actions)={majority_awards}  capture-ratchets={ratchet_captures}")
    print(f"actions  {dict(kinds)}")
    print(f"dest rings  {dict(dest_rings)}")
    print(f"capture dests  {dict(capture_dests)}")
    print(f"piece weights used  {dict(piece_weights)}")
    if end_occupied:
        print(
            f"end occupancy  mean={statistics.mean(end_occupied):.2f} "
            f"median={statistics.median(end_occupied):.1f}"
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
