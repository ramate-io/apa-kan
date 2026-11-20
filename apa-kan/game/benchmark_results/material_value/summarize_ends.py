#!/usr/bin/env python3
"""Win/draw split of every EndReason, plus material-value tiers.

The H2H ``*.txt`` summaries count each reason and split material_value /
low_value into wins/draws. This script also splits material_value by tier
(total_material, board_tiles, low_value fallback, draw) so the three-tier
rule is reconstructible from JSONL. Depths stay in the ``*.txt``
``# search_depth`` lines (those are move-weighted; per-game JSONL averages
are not).
"""

from __future__ import annotations

import gzip
import json
from collections import Counter
from pathlib import Path

REASONS = [
	"elimination",
	"locked_out",
	"center_hold",
	"petal_majority",
	"control",
	"insufficient_tiles",
	"low_value",
	"material_value",
	"turn_limit",
	"double_pass",
]

TIERS = ("total_material", "board_tiles", "low_value")


def load_games(path: Path):
	opener = gzip.open if path.name.endswith(".gz") else open
	with opener(path, "rt") as fh:
		for line in fh:
			line = line.strip()
			if line:
				yield json.loads(line)


def summarize(path: Path) -> tuple[int, Counter, Counter]:
	counts: Counter = Counter()
	tiers: Counter = Counter()
	n = 0
	for rec in load_games(path):
		n += 1
		end = rec.get("end") or {}
		status = end.get("status") or "?"
		reason = end.get("reason") or "?"
		counts[(status, reason)] += 1
		if reason != "material_value":
			continue
		mv = end.get("material_value") or {}
		kind = mv.get("kind") or "?"
		if kind == "draw" or status == "draw":
			tiers["draw"] += 1
		else:
			tiers[mv.get("tier") or "unknown"] += 1
	return n, counts, tiers


def render(title: str, n: int, counts: Counter, tiers: Counter) -> str:
	lines = [f"# {title}  games={n}"]
	for reason in REASONS:
		wins = counts.get(("won", reason), 0)
		draws = counts.get(("draw", reason), 0)
		lines.append(f"{reason:22}  wins={wins}  draws={draws}  total={wins + draws}")
	known = set(REASONS)
	for (status, reason), k in sorted(counts.items()):
		if reason not in known:
			lines.append(f"other {status}/{reason}={k}")
	mv_total = counts.get(("won", "material_value"), 0) + counts.get(
		("draw", "material_value"), 0
	)
	if mv_total:
		lines.append("# material_value_tiers")
		for tier in TIERS:
			lines.append(f"{tier:22}  wins={tiers.get(tier, 0)}")
		lines.append(f"{'draw':22}  count={tiers.get('draw', 0)}")
		unknown = {
			k: v for k, v in tiers.items() if k not in TIERS and k != "draw"
		}
		for k, v in sorted(unknown.items()):
			lines.append(f"other tier {k}={v}")
	return "\n".join(lines) + "\n"


def main() -> None:
	root = Path(__file__).resolve().parent
	n, counts, tiers = summarize(root / "h2h_shallow.jsonl.gz")
	text = render("h2h_shallow.txt material-value", n, counts, tiers)
	(root / "ends_breakdown.txt").write_text(text)
	print(text, end="")


if __name__ == "__main__":
	main()
