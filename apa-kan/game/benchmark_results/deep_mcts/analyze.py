#!/usr/bin/env python3
"""Summarize deep-MCTS JSONL for depth, root-spread, and dominant-strategy review.

Quote numbers from the committed logs, not from this script's stdout,
except as a helper for writing the PR.
"""

from __future__ import annotations

import gzip
import json
import sys
from collections import Counter, defaultdict
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


def search_label(g: dict, side: str) -> str:
    key = "p1_search" if side == "p1" else "p2_search"
    return g.get(key) or ""


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


def action_tag(act: dict) -> str:
    kind = (act.get("kind") or "?").lower()
    if kind == "add":
        return f"add@{act.get('to')}"
    if kind == "move":
        return f"move{act.get('from')}->{act.get('to')}"
    return kind


def opening_line(g: dict, n: int = 4) -> str:
    acts = g.get("actions") or []
    return " ".join(action_tag(a) for a in acts[:n])


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
    mm_avgs = [g["mm_depth_avg"] for g in logs if g.get("mm_depth_avg")]
    mm_max = max((g.get("mm_depth_max") or 0) for g in logs)
    mcts_avgs = [g["mcts_depth_avg"] for g in logs if g.get("mcts_depth_avg")]
    mcts_max = max((g.get("mcts_depth_max") or 0) for g in logs)
    mm_mean = sum(mm_avgs) / len(mm_avgs) if mm_avgs else 0.0
    mcts_mean = sum(mcts_avgs) / len(mcts_avgs) if mcts_avgs else 0.0
    print(
        f"search_depth  mm_avg={mm_mean:.2f} mm_max={mm_max}  "
        f"mcts_avg={mcts_mean:.2f} mcts_max={mcts_max}"
    )
    ent = [g.get("root_entropy_avg") or 0 for g in logs if g.get("root_eff_branch_avg")]
    eff = [g.get("root_eff_branch_avg") or 0 for g in logs if g.get("root_eff_branch_avg")]
    top1 = [g.get("root_top1_share_avg") or 0 for g in logs if g.get("root_eff_branch_avg")]
    near = [g.get("root_near_best_q_avg") or 0 for g in logs if g.get("root_eff_branch_avg")]
    qsp = [g.get("root_q_spread_top10_avg") or 0 for g in logs if g.get("root_eff_branch_avg")]
    stab = [g.get("root_best_stability_avg") or 0 for g in logs if g.get("root_eff_branch_avg")]
    legal = [g.get("root_legal_avg") or 0 for g in logs if g.get("root_eff_branch_avg")]
    if eff:
        n = len(eff)
        print(
            "root_spread  "
            f"entropy={sum(ent)/n:.2f} eff_branch={sum(eff)/n:.1f} "
            f"top1={sum(top1)/n:.2f} near_q={sum(near)/n:.1f} "
            f"q_spread10={sum(qsp)/n:.3f} stability={sum(stab)/n:.2f} "
            f"legal={sum(legal)/n:.1f}"
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

    print("MCTS opening lines (first 4 actions, MCTS as P1):")
    mcts_p1 = [g for g in logs if (g.get("p1") or "").lower() == "mcts"]
    by_line: dict[str, list] = defaultdict(list)
    for g in mcts_p1:
        by_line[opening_line(g, 4)].append(g)
    hot = sorted(by_line.items(), key=lambda kv: (-len(kv[1]), kv[0]))
    for line, gs in hot[:12]:
        ww, dd, ll = wdl(gs)
        print(f"  n={len(gs)} P1 {ww}-{dd}-{ll}  {line}")

    print("MCTS P1 first-add destinations:")
    first_add: Counter = Counter()
    first_add_wdl: dict = defaultdict(lambda: [0, 0, 0])
    for g in mcts_p1:
        acts = g.get("actions") or []
        if not acts or (acts[0].get("kind") or "").lower() != "add":
            continue
        dest = acts[0].get("to")
        first_add[dest] += 1
        w, d, l = wdl([g])
        first_add_wdl[dest][0] += w
        first_add_wdl[dest][1] += d
        first_add_wdl[dest][2] += l
    for dest, n in first_add.most_common(12):
        ww, dd, ll = first_add_wdl[dest]
        print(f"  add@{dest}  n={n} P1 {ww}-{dd}-{ll}")

    print("unusual first-adds (n>=4, decided win rate <=25% or >=75%):")
    flagged = 0
    for dest, n in first_add.most_common():
        ww, dd, ll = first_add_wdl[dest]
        decided = ww + ll
        if n < 4 or decided < 4:
            continue
        rate = ww / decided
        if rate <= 0.25 or rate >= 0.75:
            flagged += 1
            print(f"  FLAG add@{dest}  n={n} P1 {ww}-{dd}-{ll}  win_rate={rate:.2f}")
    if flagged == 0:
        print("  (none)")

    print("MCTS P2 first-add destinations (reply to P1):")
    mcts_p2 = [g for g in logs if (g.get("p2") or "").lower() == "mcts"]
    p2_add: Counter = Counter()
    p2_wdl: dict = defaultdict(lambda: [0, 0, 0])
    for g in mcts_p2:
        acts = [a for a in (g.get("actions") or []) if (a.get("kind") or "").lower() == "add"]
        if len(acts) < 2:
            continue
        dest = acts[1].get("to")
        p2_add[dest] += 1
        w, d, l = wdl([g])
        # P2 win is P1 loss.
        p2_wdl[dest][0] += l
        p2_wdl[dest][1] += d
        p2_wdl[dest][2] += w
    for dest, n in p2_add.most_common(8):
        ww, dd, ll = p2_wdl[dest]
        print(f"  add@{dest}  n={n} P2 {ww}-{dd}-{ll}")

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
        print(f"    line: {opening_line(g, 6)}")


def main() -> None:
    root = Path(__file__).resolve().parent
    names = [
        "h2h_mcts_live_deep_vs_enhanced",
        "h2h_mcts_live_enhanced_vs_deep",
        "h2h_mcts_deep_vs_enhanced",
        "h2h_mcts_enhanced_vs_deep",
        "h2h_mcts_priors_vs_enhanced",
        "h2h_mcts_enhanced_vs_priors",
        "h2h_mcts_deep_vs_mm",
        "h2h_mm_enhanced_vs_mcts_deep",
        "mcts_long",
        "mcts_long_extra",
    ]
    args = sys.argv[1:] or names
    for name in args:
        path = Path(name)
        if not path.is_absolute() and not path.exists():
            path = root / name
        if path.suffix == ".txt":
            print(path.read_text())
            continue
        if not str(path).endswith(".gz"):
            path = Path(str(path) + ".jsonl.gz")
        if not path.exists():
            print(f"missing {path}")
            continue
        label = path.stem.replace(".jsonl", "")
        summarize(label, load(path))


if __name__ == "__main__":
    main()
