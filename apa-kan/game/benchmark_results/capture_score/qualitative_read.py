#!/usr/bin/env python3
"""Short qualitative read of capture-score JSONL for DEEP_PROBE.md."""

from __future__ import annotations

import gzip
import json
import statistics
import sys
from collections import Counter
from pathlib import Path

CENTER = 0
INNER = {9, 1, 12, 3, 16, 6, 18, 21}


def load(path: Path) -> list[dict]:
    with gzip.open(path, "rt") as f:
        return [json.loads(line) for line in f if line.strip()]


def ring(node: int) -> str:
    if node == CENTER:
        return "center"
    if node in INNER:
        return "inner"
    return "outer"


def piece_label(p: dict | None) -> str:
    if not p:
        return "?"
    w = int(p.get("n_rings") or p.get("weight") or 0)
    b = int(p.get("braid_rings") or 0)
    z = int(p.get("zag_rings") or 0)
    bead = int(p.get("bead_rings") or 0)
    norm = int(p.get("normal_rings") or 0)
    cap = p.get("capture") or "dot"
    if b:
        kind = f"braid{b}"
    elif z:
        kind = f"zag{z}"
    elif bead:
        kind = f"bead{bead}"
    else:
        kind = f"norm{norm}"
    return f"{kind} w{w} {cap}"


def summarize(path: Path) -> None:
    games = load(path)
    name = path.name
    print(f"\n### {name} (n={len(games)} games)")
    for g in games:
        end = g.get("end") or {}
        print(
            f"  game {g.get('game')} seed={g.get('seed')} turns={g.get('turns')} "
            f"end={end.get('status')}:{end.get('reason')} "
            f"score={end.get('p1_score')}-{end.get('p2_score')}"
        )
    center_occ_turns: list[int] = []
    center_adds = 0
    center_moves = 0
    add_rings: Counter[str] = Counter()
    move_rings: Counter[str] = Counter()
    capture_kinds: Counter[str] = Counter()
    reverse_ping = 0
    prev_move = None
    claim_count = 0

    for g in games:
        for act in g.get("actions") or []:
            kind = (act.get("kind") or "").upper()
            to = act.get("to")
            fr = act.get("from")
            if kind == "ADD" and to == CENTER:
                center_adds += 1
            if kind == "MOVE" and (to == CENTER or fr == CENTER):
                center_moves += 1
            if kind == "CLAIM":
                claim_count += 1
            dest = to if to is not None else fr
            if dest is not None:
                pass
            piece = act.get("piece")
            lab = piece_label(piece)
            if kind == "ADD":
                add_rings[lab] += 1
            elif kind == "MOVE":
                move_rings[lab] += 1
            for cap in act.get("captures") or []:
                capture_kinds[piece_label(cap.get("piece"))] += 1
            if kind == "MOVE" and fr is not None and to is not None:
                mv = (fr, to)
                if prev_move == (to, fr):
                    reverse_ping += 1
                prev_move = mv
        final = g.get("final_state") or {}
        on_center = any(
            t.get("node") == CENTER
            for side in ("p1_board", "p2_board")
            for t in (final.get(side) or [])
        )
        if on_center:
            center_occ_turns.append(g.get("turns") or 0)

    actions = sum(len(g.get("actions") or []) for g in games)
    dest_rings: Counter[str] = Counter()
    for g in games:
        for act in g.get("actions") or []:
            to = act.get("to")
            if to is not None:
                dest_rings[ring(int(to))] += 1

    print(f"  dest rings: {dict(dest_rings)}")
    print(f"  center traffic: ADD={center_adds} MOVE_touching_center={center_moves} CLAIM={claim_count}")
    print(f"  games ending with center occupied: {len(center_occ_turns)}/{len(games)}")
    print(f"  reverse-move pairs (consecutive A->B then B->A): {reverse_ping}")
    print(f"  top ADD kinds: {add_rings.most_common(6)}")
    print(f"  top MOVE kinds: {move_rings.most_common(6)}")
    opp_caps = []
    for g in games:
        n = 0
        for act in g.get("actions") or []:
            p = act.get("player")
            for cap in act.get("captures") or []:
                if cap.get("owner") is not None and p is not None and int(cap["owner"]) != int(p):
                    n += 1
        opp_caps.append(n)
    if opp_caps:
        print(
            f"  opp captures/game: mean={statistics.mean(opp_caps):.1f} "
            f"median={statistics.median(opp_caps):.1f}"
        )


def main() -> int:
    paths = [Path(p) for p in sys.argv[1:]] or [
        Path(__file__).parent / "smoke_base_mm.jsonl.gz"
    ]
    for p in paths:
        if not p.exists():
            print(f"missing {p}", file=sys.stderr)
            continue
        summarize(p)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())