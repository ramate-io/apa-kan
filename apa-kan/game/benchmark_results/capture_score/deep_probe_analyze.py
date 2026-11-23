#!/usr/bin/env python3
"""Summarize apa-kan-bench --deep-probe TSV output."""

from __future__ import annotations

import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path


def load(path: Path) -> tuple[dict[str, str], list[dict[str, str]]]:
    meta: dict[str, str] = {}
    rows: list[dict[str, str]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        if line.startswith("# "):
            if "=" in line and not line.startswith("# log="):
                k, _, v = line[2:].partition("=")
                meta[k.strip()] = v.strip()
            continue
        if not line.startswith("pos\t"):
            continue
        parts = line.split("\t")
        row: dict[str, str] = {}
        i = 0
        while i < len(parts):
            key = parts[i]
            if i + 1 < len(parts) and not parts[i + 1].startswith("m") and key in {
                "pos",
                "co",
                "cs",
                "ic",
                "sw",
                "fan16_n",
                "fan16_moves",
                "d16",
                "d64",
                "ch16",
                "ch64",
            }:
                row[key] = parts[i + 1]
                i += 2
                continue
            if key.startswith("m") and ":" in key:
                row[key] = parts[i + 1] if i + 1 < len(parts) else ""
                i += 2
                continue
            if i == 0:
                row["_lead"] = "\t".join(parts)
            i += 1
        # Re-parse structured prefix
        cols = line.split("\t")
        if len(cols) >= 9 and cols[0] == "pos":
            row = {
                "log": cols[1],
                "game": cols[2],
                "ix": cols[3],
                "turn": cols[4],
                "player": cols[5],
                "score": cols[6],
                "co": cols[7].split("=")[-1],
                "cs": cols[8].split("=")[-1],
            }
            for c in cols[9:]:
                if c.startswith("ic="):
                    row["ic"] = c.split("=")[-1]
                elif c.startswith("sw="):
                    row["sw"] = c.split("=")[-1]
                elif c.startswith("ch16="):
                    row["ch16"] = c.split("=")[-1]
                elif c.startswith("ch64="):
                    row["ch64"] = c.split("=")[-1]
                elif c.startswith("d64="):
                    row["d64"] = c.split("=")[-1]
                elif c.startswith("term1") or "term1" in c:
                    pass
                elif c.startswith("m64:"):
                    row["m64"] = c
                elif c.startswith("m1:"):
                    row["m1"] = c
            rows.append(row)
    return meta, rows


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: deep_probe_analyze.py <probe_out.txt>", file=sys.stderr)
        return 2
    path = Path(sys.argv[1])
    meta, rows = load(path)
    print(f"# analyze {path.name} positions={len(rows)}")
    if meta:
        print("meta", meta)
    if not rows:
        return 0
    ch16 = [int(r.get("ch16", "0")) for r in rows if "ch16" in r]
    ch64 = [int(r.get("ch64", "0")) for r in rows if "ch64" in r]
    depths = [int(r["d64"]) for r in rows if r.get("d64", "").isdigit()]
    ic = sum(int(r.get("ic", "0")) for r in rows)
    by_log: dict[str, list] = defaultdict(list)
    for r in rows:
        by_log[r.get("log", "?")].append(r)
    print(f"change_rate_16={sum(ch16)/len(ch16):.3f} change_rate_64={sum(ch64)/len(ch64):.3f}")
    if depths:
        print(
            f"id_depth_64x mean={statistics.mean(depths):.2f} "
            f"median={statistics.median(depths):.1f} max={max(depths)}"
        )
    print(f"immediate_capture_positions={ic}/{len(rows)}")
    for k, rs in sorted(by_log.items()):
        print(f"  {k}: n={len(rs)} ch64={sum(int(x.get('ch64',0)) for x in rs)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())