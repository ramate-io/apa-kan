#!/usr/bin/env python3
"""Survey 2-2-1-1 opening JSONL.gz logs.

Reuses the cascade_winner_swap survey (seat, length, ratchets, lockouts,
comebacks), then adds how the opening doubles were spent: kinds, dest rings,
whether P1/P2 touched the origin on their double, and whether P2's reply
punished P1 (P2 takes 0 during/right after its double).
"""

from __future__ import annotations

import importlib.util
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load {path}")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


base = _load("exactplay_survey", HERE.parent / "exactplay" / "survey.py")
# cascade_winner_swap/survey.py does `import survey as base`; the script dir
# is this folder, so pin exactplay's survey before loading that file.
sys.modules["survey"] = base
ratchet = _load(
    "cascade_winner_swap_survey",
    HERE.parent / "cascade_winner_swap" / "survey.py",
)

CENTER = base.CENTER
INNER = base.INNER


def dest_ring(node) -> str:
    if node == CENTER:
        return "center"
    if node in INNER:
        return "inner"
    return "outer"


def opening_survey(games: list[dict]) -> None:
    n = len(games)
    p1_kinds = Counter()
    p2_kinds = Counter()
    p1_dest = Counter()
    p2_dest = Counter()
    p1_opening_n = []
    p2_opening_n = []
    p1_touched_origin = 0
    p2_touched_origin = 0
    p2_takes_origin_in_double = 0
    p2_takes_origin_reply = 0  # P2 takes 0 on its double while P1 had occupied 0
    p1_two_adds = 0
    p2_two_adds = 0
    interrupted = 0  # P1 opening count != 2
    examples = []

    for g in games:
        actions = g.get("actions") or []
        p1_open = [a for a in actions if a.get("player") == 1 and a.get("played_as_opening")]
        p2_open = [a for a in actions if a.get("player") == 2 and a.get("played_as_opening")]
        p1_opening_n.append(len(p1_open))
        p2_opening_n.append(len(p2_open))
        if len(p1_open) != 2:
            interrupted += 1
        p1_add = 0
        p2_add = 0
        p1_origin = False
        p2_origin = False
        p1_on_origin = False
        occ = None
        for act in actions:
            kind = act.get("kind")
            player = act.get("player")
            to = act.get("to")
            frm = act.get("from")
            is_open = bool(act.get("played_as_opening"))
            if is_open and player == 1:
                p1_kinds[kind or "?"] += 1
                p1_dest[dest_ring(to)] += 1
                if kind == "ADD":
                    p1_add += 1
                if to == CENTER or frm == CENTER:
                    p1_origin = True
            if is_open and player == 2:
                p2_kinds[kind or "?"] += 1
                p2_dest[dest_ring(to)] += 1
                if kind == "ADD":
                    p2_add += 1
                if to == CENTER or frm == CENTER:
                    p2_origin = True
                if to == CENTER:
                    p2_takes_origin_in_double += 1
                    if p1_on_origin:
                        p2_takes_origin_reply += 1
            if kind in ("ADD", "MOVE", "SWAP", "CONVERT", "CLAIM") and to == CENTER:
                occ = player
            elif kind == "MOVE" and frm == CENTER:
                occ = None
            if occ == 1:
                p1_on_origin = True
            elif occ != 1:
                p1_on_origin = False
        if p1_origin:
            p1_touched_origin += 1
        if p2_origin:
            p2_touched_origin += 1
        if p1_add == 2:
            p1_two_adds += 1
        if p2_add == 2:
            p2_two_adds += 1
        if len(examples) < 8:
            examples.append(
                {
                    "pairing": g.get("pairing"),
                    "game": g.get("game"),
                    "seed": g.get("seed"),
                    "p1": [
                        (a.get("kind"), a.get("to"), dest_ring(a.get("to")))
                        for a in p1_open
                    ],
                    "p2": [
                        (a.get("kind"), a.get("to"), dest_ring(a.get("to")))
                        for a in p2_open
                    ],
                    "winner": (g.get("end") or {}).get("winner"),
                    "turns": g.get("turns"),
                    "reason": (g.get("end") or {}).get("reason"),
                    "p1_n": len(p1_open),
                    "p2_n": len(p2_open),
                }
            )

    print("\n# 2-2-1-1 opening doubles")
    print(
        f"  P1 opening actions mean={base.mean(p1_opening_n):.2f}  "
        f"n=2 in {sum(1 for x in p1_opening_n if x == 2)}/{n}  "
        f"interrupted (P1 n!=2) {interrupted}/{n}"
    )
    print(
        f"  P2 opening actions mean={base.mean(p2_opening_n):.2f}  "
        f"n=2 in {sum(1 for x in p2_opening_n if x == 2)}/{n}"
    )
    print(f"  P1 kinds {dict(p1_kinds)}  dest {dict(p1_dest)}")
    print(f"  P2 kinds {dict(p2_kinds)}  dest {dict(p2_dest)}")
    print(
        f"  both-ADD doubles  P1 {p1_two_adds}/{n} ({base.pct(p1_two_adds, n)})  "
        f"P2 {p2_two_adds}/{n} ({base.pct(p2_two_adds, n)})"
    )
    print(
        f"  touched origin on double  P1 {p1_touched_origin}/{n} "
        f"({base.pct(p1_touched_origin, n)})  "
        f"P2 {p2_touched_origin}/{n} ({base.pct(p2_touched_origin, n)})"
    )
    print(
        f"  P2 takes origin during its double {p2_takes_origin_in_double}/{n}  "
        f"of those as a reply to P1-on-0 {p2_takes_origin_reply}"
    )
    print("  sample openings:")
    for ex in examples:
        print(
            f"    {ex['pairing']} game {ex['game']} seed {ex['seed']} "
            f"P1 {ex['p1']} P2 {ex['p2']} n={ex['p1_n']}/{ex['p2_n']} "
            f"end={ex['reason']} W={ex['winner']} turns={ex['turns']}"
        )


def summarize_one(path: Path) -> None:
    games = base.load_games(path)
    print(f"===== {path} =====")
    base.summarize(path)
    ratchet.extra_survey(games)
    opening_survey(games)
    print()


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: survey.py <games.jsonl.gz> [...]", file=sys.stderr)
        sys.exit(2)
    for arg in sys.argv[1:]:
        summarize_one(Path(arg))


if __name__ == "__main__":
    main()
