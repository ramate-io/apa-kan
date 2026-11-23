#!/usr/bin/env python3
"""Compare deep-wide search logs vs standard deep exactplay.

Reports unique action types, same-deal prefix disagreements, and citations
for CLAIM / SWAP / single-braid center hits / edge play / stalling.
"""

from __future__ import annotations

import gzip
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

CENTER = 0
INNER = {9, 1, 12, 3, 16, 6, 18, 21}


def load_games(path: Path) -> list[dict]:
    opener = gzip.open if path.name.endswith(".gz") else open
    games = []
    with opener(path, "rt", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                games.append(json.loads(line))
    return games


def act_key(act: dict) -> tuple:
    piece = act.get("piece") or {}
    return (
        act.get("kind"),
        act.get("player"),
        act.get("from"),
        act.get("to"),
        piece.get("n_rings"),
        piece.get("braid_rings"),
        piece.get("normal_rings"),
        act.get("bench_index"),
    )


def is_sb_center_pop(act: dict) -> bool:
    piece = act.get("piece") or {}
    if (piece.get("n_rings") or 0) != 1 or (piece.get("braid_rings") or 0) <= 0:
        return False
    return any(
        c.get("node") == CENTER and c.get("owner") != act.get("player")
        for c in (act.get("captures") or [])
    )


def is_edge(node) -> bool:
    return node is not None and node != CENTER and node not in INNER


def cite(path: Path, g: dict) -> str:
    return f"{path.name} / {g.get('pairing')} / game {g.get('game')} / {g.get('turns')}t"


def summarize(path: Path, games: list[dict]) -> dict:
    n = len(games)
    p1 = sum(1 for g in games if (g.get("end") or {}).get("winner") == 1)
    p2 = sum(1 for g in games if (g.get("end") or {}).get("winner") == 2)
    draws = n - p1 - p2
    reasons = Counter((g.get("end") or {}).get("reason") for g in games)
    kinds = Counter()
    claims = swaps = center_swaps = sb_pops = edge_adds = edge_moves = zb = actions = 0
    pass_n = 0
    t200 = 0
    unusual_picks = []
    citations = defaultdict(list)
    for g in games:
        turns = int(g.get("turns") or 0)
        if turns >= 200:
            t200 += 1
            citations["stall_t200"].append(cite(path, g))
        opening = g.get("opening") or {}
        for seat, key in ((1, "p1_bench"), (2, "p2_bench")):
            bench = opening.get(key) or []
            ones = [t for t in bench if isinstance(t, dict) and int(t.get("n_rings") or 0) == 1]
            braids = [t for t in ones if int(t.get("braid_rings") or 0) > 0]
            if len(ones) >= 4:
                unusual_picks.append((cite(path, g), seat, "1-ring-heavy", len(ones), len(bench)))
            if braids:
                unusual_picks.append(
                    (cite(path, g), seat, "single-braid-bench", len(braids), len(bench))
                )
        for act in g.get("actions") or []:
            actions += 1
            kind = act.get("kind")
            kinds[kind] += 1
            if kind == "CLAIM":
                claims += 1
                citations["CLAIM"].append(
                    f"{cite(path, g)} t{act.get('turn')} P{act.get('player')} node={act.get('to') or act.get('from')}"
                )
            if kind == "SWAP":
                swaps += 1
                if act.get("to") == CENTER or act.get("from") == CENTER:
                    center_swaps += 1
                    citations["center_SWAP"].append(
                        f"{cite(path, g)} t{act.get('turn')} P{act.get('player')} {act.get('from')}→{act.get('to')}"
                    )
                else:
                    citations["SWAP"].append(
                        f"{cite(path, g)} t{act.get('turn')} P{act.get('player')} {act.get('from')}→{act.get('to')}"
                    )
            if kind == "PASS":
                pass_n += 1
                citations["PASS"].append(f"{cite(path, g)} t{act.get('turn')} P{act.get('player')}")
            if is_sb_center_pop(act):
                sb_pops += 1
                citations["SB_center_pop"].append(
                    f"{cite(path, g)} t{act.get('turn')} P{act.get('player')} {act.get('from')}→{act.get('to')}"
                )
            if kind == "ADD" and is_edge(act.get("to")):
                edge_adds += 1
            if kind == "MOVE" and is_edge(act.get("to")) and act.get("from") in INNER | {CENTER}:
                edge_moves += 1
                citations["edge_from_inner"].append(
                    f"{cite(path, g)} t{act.get('turn')} P{act.get('player')} {act.get('from')}→{act.get('to')}"
                )
            if act.get("budget") == 0:
                zb += 1
    return {
        "file": str(path),
        "n": n,
        "p1_wdl": f"{p1}-{draws}-{p2}",
        "reasons": dict(reasons),
        "t200": t200,
        "kinds": dict(kinds),
        "claims": claims,
        "swaps": swaps,
        "center_swaps": center_swaps,
        "sb_pops": sb_pops,
        "edge_adds": edge_adds,
        "edge_from_inner": edge_moves,
        "pass": pass_n,
        "zb_share": (zb / actions) if actions else 0.0,
        "citations": {k: v[:8] for k, v in citations.items()},
        "unusual_picks": unusual_picks[:8],
        "games": games,
    }


def index_by_deal(games: list[dict]) -> dict:
    out = {}
    for g in games:
        out[(g.get("pairing"), g.get("game"))] = g
    return out


def compare_same_deals(std: list[dict], wide: list[dict], std_path: Path, wide_path: Path) -> None:
    si = index_by_deal(std)
    wi = index_by_deal(wide)
    shared = sorted(set(si) & set(wi))
    print(f"\n# same-deal prefix vs {std_path.name}  ({len(shared)} shared)")
    first_dis = []
    outcome_flip = 0
    for key in shared:
        a, b = si[key], wi[key]
        aa, ba = a.get("actions") or [], b.get("actions") or []
        ply = 0
        for x, y in zip(aa, ba):
            if act_key(x) != act_key(y):
                break
            ply += 1
        else:
            ply = min(len(aa), len(ba))
        aw = (a.get("end") or {}).get("winner")
        bw = (b.get("end") or {}).get("winner")
        if aw != bw:
            outcome_flip += 1
        if ply < min(len(aa), len(ba)) or aw != bw:
            dis_act = ba[ply] if ply < len(ba) else {}
            first_dis.append(
                (
                    key,
                    ply,
                    aw,
                    bw,
                    dis_act.get("kind"),
                    dis_act.get("from"),
                    dis_act.get("to"),
                    int(b.get("turns") or 0),
                )
            )
    print(f"  outcome flips {outcome_flip}/{len(shared)}")
    print("  first disagreements (pairing, game, ply, stdW, wideW, kind, from, to, turns):")
    for row in first_dis[:12]:
        print(f"    {row}")


def kinds_never_in_std(std_games: list[dict], wide_games: list[dict], label: str) -> None:
    def bag(games):
        s = set()
        for g in games:
            for act in g.get("actions") or []:
                k = act.get("kind")
                if k in ("CLAIM", "SWAP", "PASS"):
                    s.add((k, act.get("from"), act.get("to")))
                if is_sb_center_pop(act):
                    s.add(("SB_CENTER", act.get("from"), act.get("to")))
        return s

    extra = bag(wide_games) - bag(std_games)
    print(f"\n# {label}: action signatures in wide but not in standard deep corpus: {len(extra)}")
    for item in sorted(extra, key=lambda x: (x[0] or "", x[1] or -1, x[2] or -1))[:20]:
        print(f"  {item}")


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: analyze_wide.py <wide.jsonl.gz> [...] [--std <deep4x_self.jsonl.gz>]", file=sys.stderr)
        sys.exit(2)
    args = sys.argv[1:]
    std_path = None
    if "--std" in args:
        i = args.index("--std")
        std_path = Path(args[i + 1])
        del args[i : i + 2]
    std_games = load_games(std_path) if std_path else []
    wide_all = []
    for arg in args:
        path = Path(arg)
        games = load_games(path)
        wide_all.extend(games)
        s = summarize(path, games)
        print(f"file {path.name}")
        print(
            f"  games {s['n']}  P1 {s['p1_wdl']}  reasons {s['reasons']}  t200 {s['t200']}"
        )
        print(
            f"  CLAIM {s['claims']}  SWAP {s['swaps']} (center {s['center_swaps']})  "
            f"SB pops {s['sb_pops']}  PASS {s['pass']}  "
            f"edge ADD {s['edge_adds']}  inner→edge MOVE {s['edge_from_inner']}  "
            f"ZB {s['zb_share']*100:.1f}%"
        )
        print(f"  kinds {s['kinds']}")
        for k, cites in s["citations"].items():
            if cites:
                print(f"  cite {k}:")
                for c in cites:
                    print(f"    {c}")
        if s["unusual_picks"]:
            print("  unusual picks:")
            for row in s["unusual_picks"]:
                print(f"    {row}")
        print()
        if std_games:
            compare_same_deals(std_games, games, std_path, path)
            kinds_never_in_std(std_games, games, path.name)


if __name__ == "__main__":
    main()
