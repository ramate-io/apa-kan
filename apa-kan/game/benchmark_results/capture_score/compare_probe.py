#!/usr/bin/env python3
"""Compare two deep_probe_results TSV files."""

from __future__ import annotations

import re
import sys
from pathlib import Path


def load_summary(path: Path) -> dict[str, str]:
    text = path.read_text()
    out: dict[str, str] = {}
    for line in text.splitlines():
        if line.startswith("move_change_rate"):
            out["move_change"] = line.strip()
        elif line.startswith("positions="):
            out["positions"] = line.strip()
        elif line.startswith("deep_only_terminal"):
            out["terminal"] = line.strip()
        elif line.startswith("imm_capture"):
            out["imm"] = line.strip()
        elif line.startswith("fanout_unique"):
            out["fanout"] = line.strip()
        elif line.startswith("dest_ring_light"):
            out["rings_light"] = line.strip()
        elif line.startswith("dest_ring_max"):
            out["rings_deep"] = line.strip()
    rows = [l for l in text.splitlines() if l.startswith("pos\t")]
    out["n_rows"] = str(len(rows))
    uniq = []
    for row in rows:
        m = re.search(r"fan(\d+)_uniq=(\d+)", row)
        if m:
            uniq.append(int(m.group(2)))
    if uniq:
        out["fan_uniq_per_row"] = f"mean={sum(uniq)/len(uniq):.2f} max={max(uniq)}"
    ch64 = sum(1 for row in rows if "\tch64=1" in row)
    ch16 = sum(1 for row in rows if "\tch16=1" in row)
    if rows:
        out["ch16_count"] = f"{ch16}/{len(rows)}"
        out["ch64_count"] = f"{ch64}/{len(rows)}"
    return out


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: compare_probe.py <off.txt> <fanout.txt>")
        return 2
    a, b = Path(sys.argv[1]), Path(sys.argv[2])
    sa, sb = load_summary(a), load_summary(b)
    print(f"# compare\n# A {a.name}\n# B {b.name}")
    for key in sorted(set(sa) | set(sb)):
        print(f"  {key}:  A={sa.get(key,'?')}  |  B={sb.get(key,'?')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())