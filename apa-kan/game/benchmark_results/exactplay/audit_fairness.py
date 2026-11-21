#!/usr/bin/env python3
"""Seat balance, draw/lockout, length, and dead-mechanic audit for JSONL logs."""

from __future__ import annotations

import gzip
import json
import math
import sys
from collections import Counter, defaultdict
from pathlib import Path

CENTER = 0


def act_sig(act: dict | None) -> tuple:
    if not act:
        return ()
    piece = act.get("piece") or {}
    return (
        act.get("kind"),
        act.get("player"),
        act.get("from"),
        act.get("to"),
        piece.get("id"),
        act.get("bench_index"),
    )


def load_games(path: Path) -> list[dict]:
    opener = gzip.open if path.name.endswith(".gz") else open
    games = []
    with opener(path, "rt", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                games.append(json.loads(line))
    return games


def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float, float]:
    if n <= 0:
        return 0.0, 0.0, 0.0
    p = k / n
    den = 1.0 + z * z / n
    centre = (p + z * z / (2 * n)) / den
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / den
    return p, max(0.0, centre - half), min(1.0, centre + half)


def audit(path: Path) -> None:
    games = load_games(path)
    n = len(games)
    if not n:
        print(f"{path}: empty")
        return
    p1 = sum(1 for g in games if (g.get("end") or {}).get("winner") == 1)
    p2 = sum(1 for g in games if (g.get("end") or {}).get("winner") == 2)
    draws = n - p1 - p2
    reasons = Counter((g.get("end") or {}).get("reason") for g in games)
    turns = [int(g.get("turns") or 0) for g in games]
    turn200 = sum(1 for t in turns if t >= 200)
    lock = reasons.get("locked_out", 0)
    claims = swaps = center_swaps = sb_pops = zb = actions = 0
    pick_p1 = []
    for g in games:
        opening = g.get("opening") or {}
        setup = g.get("setup") or {}
        p1_bench = setup.get("p1_bench") or opening.get("p1_bench") or []
        if isinstance(p1_bench, list) and p1_bench and isinstance(p1_bench[0], dict):
            rings = [int(t.get("n_rings") or 0) for t in p1_bench]
            pick_p1.append(sum(1 for r in rings if r == 1) / max(len(rings), 1))
        for act in g.get("actions") or []:
            actions += 1
            kind = act.get("kind")
            if kind == "CLAIM":
                claims += 1
            if kind == "SWAP":
                swaps += 1
                if act.get("to") == CENTER or act.get("from") == CENTER:
                    center_swaps += 1
            piece = act.get("piece") or {}
            if (
                (piece.get("n_rings") or 0) == 1
                and (piece.get("braid_rings") or 0) > 0
                and any(
                    c.get("node") == CENTER and c.get("owner") != act.get("player")
                    for c in (act.get("captures") or [])
                )
            ):
                sb_pops += 1
            if act.get("budget") == 0:
                zb += 1
    p, lo, hi = wilson(p1, n)
    print(f"file {path}")
    print(
        f"  games {n}  P1 {p1}-{draws}-{p2}  P1% {p*100:.1f}  "
        f"Wilson95 [{lo*100:.1f},{hi*100:.1f}]"
    )
    print(
        f"  turns mean {sum(turns)/n:.1f}  median {sorted(turns)[n//2]}  "
        f"min {min(turns)} max {max(turns)}  t200 {turn200}/{n}"
    )
    print(f"  reasons {dict(reasons)}  lockout {lock}/{n} ({lock/n*100:.1f}%)")
    print(
        f"  CLAIM {claims} ({claims/n:.2f}/g)  SWAP {swaps} ({swaps/n:.2f}/g)  "
        f"center SWAP {center_swaps}  SB center pops {sb_pops}  "
        f"ZB {zb}/{actions} ({zb/actions*100 if actions else 0:.1f}%)"
    )
    if pick_p1:
        print(f"  P1 1-ring bench share mean {sum(pick_p1)/len(pick_p1)*100:.1f}%")
    by_pair = defaultdict(lambda: [0, 0, 0])
    for g in games:
        pair = g.get("pairing") or f"{g.get('p1')} vs {g.get('p2')}"
        w = (g.get("end") or {}).get("winner")
        if w == 1:
            by_pair[pair][0] += 1
        elif w == 2:
            by_pair[pair][2] += 1
        else:
            by_pair[pair][1] += 1
    if by_pair:
        print("  by pairing (P1 W-D-L):")
        for pair, (w, d, l) in sorted(by_pair.items()):
            n_p = w + d + l
            pw, lo, hi = wilson(w, n_p)
            print(
                f"    {pair}: {w}-{d}-{l}  P1% {pw*100:.1f} [{lo*100:.1f},{hi*100:.1f}]"
            )
    first_kinds = Counter()
    grouped = defaultdict(list)
    for g in games:
        grouped[(g.get("seed"), g.get("game"))].append(g)
        acts = g.get("actions") or []
        if acts:
            first_kinds[acts[0].get("kind")] += 1
    deck_n = deck_same = 0
    first_act_n = first_act_same = 0
    for _key, gs in grouped.items():
        if len(gs) < 2:
            continue
        deck_n += 1
        d0 = (gs[0].get("setup") or {}).get("p1_deck")
        if all((g.get("setup") or {}).get("p1_deck") == d0 for g in gs[1:]):
            deck_same += 1
        first_act_n += 1
        k0 = act_sig((gs[0].get("actions") or [None])[0])
        if all(act_sig((g.get("actions") or [None])[0]) == k0 for g in gs[1:]):
            first_act_same += 1
    if deck_n:
        print(f"  same-deck 27 identical across pairings {deck_same}/{deck_n}")
        print(
            f"  first action identical across pairings {first_act_same}/{first_act_n} "
            "(engines differ, so this is not deck luck)"
        )
    if first_kinds:
        print(f"  first action kinds {dict(first_kinds)}")
    print()


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: audit_fairness.py <jsonl.gz> [...]", file=sys.stderr)
        sys.exit(2)
    for arg in sys.argv[1:]:
        audit(Path(arg))


if __name__ == "__main__":
    main()
