#!/usr/bin/env python3
"""Summarize bead-scoring JSONL for dominant-strategy review.

Quote numbers from the committed logs, not from this script's stdout,
except as a helper for writing the PR. Deep-eval priority: an archetype
or line that wins regardless of seat / opponent.
"""

from __future__ import annotations

import gzip
import json
import sys
from collections import defaultdict
from pathlib import Path


def load(path: Path) -> list[dict]:
    with gzip.open(path, "rt") as f:
        return [json.loads(line) for line in f if line.strip()]


def wdl(logs: list[dict]) -> tuple[int, int, int]:
    w = d = l = 0
    for g in logs:
        st = g["end"]["status"]
        if st == "won":
            if g["end"].get("winner") == 1:
                w += 1
            else:
                l += 1
        else:
            d += 1
    return w, d, l


def reason(g: dict) -> str:
    return g["end"].get("reason") or g["end"]["status"]


def matchup(g: dict) -> str:
    return g.get("deck_matchup") or "mixed vs mixed"


def pairing(g: dict) -> str:
    return g.get("pairing") or f"{g.get('p1')} vs {g.get('p2')}"


def bench_kinds(g: dict, side: str) -> str:
    setup = g.get("setup") or {}
    key = "p1_pick7" if side == "p1" else "p2_pick7"
    ext = "p1_extend" if side == "p1" else "p2_extend"
    tiles = list(setup.get(key) or []) + list(setup.get(ext) or [])
    if not tiles:
        tiles = list((g.get("opening") or {}).get(f"{side}_bench") or [])
    parts = []
    for t in tiles:
        rings = []
        for r in t.get("rings") or []:
            tag = r.get("c", "?")
            if r.get("b"):
                tag += "d"
            elif r.get("z"):
                tag += "z"
            elif r.get("e"):
                tag += "e"
            rings.append(tag)
        mark = "dot" if t.get("capture") == "dot" else "tri"
        parts.append(f"{'+'.join(rings) or '?'}d{t.get('dots', 0)}{mark[0]}")
    return ", ".join(parts)


def summarize(name: str, logs: list[dict]) -> None:
    print(f"\n## {name}  n={len(logs)}")
    w, d, l = wdl(logs)
    turns = [g["turns"] for g in logs]
    mean_t = sum(turns) / len(turns) if turns else 0
    print(f"P1 {w}-{d}-{l}  mean_turns={mean_t:.2f}")
    ends = defaultdict(int)
    for g in logs:
        ends[reason(g)] += 1
    print("ends", dict(ends))
    contrib = sum(g.get("bead_contrib_captures") or 0 for g in logs)
    needed = sum(g.get("bead_needed_captures") or 0 for g in logs)
    spawn = sum(g.get("spawn_two_ring_single_bead") or 0 for g in logs)
    caps = sum(sum(a.get("capture", 0) for a in g.get("actions") or []) for g in logs)
    print(
        f"captures opp={caps}  bead_contrib={contrib} needed={needed} "
        f"spawn_two_ring_single_bead={spawn}"
    )

    print("by pairing:")
    by_p: dict[str, list] = defaultdict(list)
    for g in logs:
        by_p[pairing(g)].append(g)
    for k, gs in sorted(by_p.items()):
        ww, dd, ll = wdl(gs)
        print(f"  {k:32}  P1 {ww}-{dd}-{ll}  mean_t={sum(x['turns'] for x in gs)/len(gs):.1f}")

    print("by matchup:")
    by_m: dict[str, list] = defaultdict(list)
    for g in logs:
        by_m[matchup(g)].append(g)
    for k, gs in sorted(by_m.items()):
        ww, dd, ll = wdl(gs)
        print(f"  {k:20}  P1 {ww}-{dd}-{ll}  mean_t={sum(x['turns'] for x in gs)/len(gs):.1f}")
        lock = sum(1 for x in gs if reason(x) == "locked_out")
        ctrl = sum(1 for x in gs if reason(x) == "control")
        short = [x for x in gs if x["turns"] <= 20 and x["end"]["status"] == "won"]
        print(f"    lockout={lock} control={ctrl} short_wins(<=20)={len(short)}")

    # Archetype seat: if "braid vs bead" P1 (braid) and "bead vs braid" P2 (braid)
    # both crush, braid is dominant regardless of seat.
    print("cross-matchup (dominant-strategy check):")
    for a, b in (("braid", "bead"), ("bead", "braid")):
        key = f"{a} vs {b}"
        gs = by_m.get(key, [])
        if not gs:
            continue
        ww, dd, ll = wdl(gs)
        print(f"  {key}: P1={a} {ww}-{dd}-{ll}  (P2={b})")
        if ww + ll >= 4 and ww / (ww + ll) >= 0.75:
            print(f"    NOTE: {a} as P1 won {ww}/{ww+ll} decided games vs {b}")
        if ww + ll >= 4 and ll / (ww + ll) >= 0.75:
            print(f"    NOTE: {b} as P2 won {ll}/{ww+ll} decided games vs {a}")

    print("notable games (short wins, lockouts, one-sided):")
    notable = []
    for g in logs:
        tags = []
        if g["turns"] <= 20 and g["end"]["status"] == "won":
            tags.append("short")
        if reason(g) == "locked_out":
            tags.append("lockout")
        if g.get("spawn_two_ring_single_bead"):
            tags.append(f"spawn2ring={g['spawn_two_ring_single_bead']}")
        if g["turns"] >= 150:
            tags.append("long")
        if tags:
            notable.append((tags, g))
    for tags, g in notable:
        winner = g["end"].get("winner")
        print(
            f"  {pairing(g)} [{matchup(g)}] g{g['game']} seed={g['seed']} "
            f"P{winner or 'D'} {reason(g)} t={g['turns']} {tags}"
        )
        print(f"    P1 bench: {bench_kinds(g, 'p1')}")
        print(f"    P2 bench: {bench_kinds(g, 'p2')}")


def main() -> None:
    root = Path(__file__).resolve().parent
    names = [
        "independent_shallow",
        "independent_deep",
        "shared_shallow",
        "shared_deep",
    ]
    args = sys.argv[1:] or names
    for name in args:
        path = root / f"{name}.jsonl.gz" if not name.endswith(".gz") else Path(name)
        if not path.exists():
            print(f"missing {path}")
            continue
        summarize(path.stem.replace(".jsonl", ""), load(path))


if __name__ == "__main__":
    main()
