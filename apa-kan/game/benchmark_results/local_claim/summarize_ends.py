#!/usr/bin/env python3
"""Win/draw split of every EndReason from per-game JSONL (gzip ok).

The H2H ``*.txt`` summaries count each reason and split low_value into
wins/draws. This script splits every reason so elim/insuff/etc. draws
are visible. Depths stay in the ``*.txt`` ``# search_depth`` lines
(those are move-weighted; per-game JSONL averages are not).
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
	"turn_limit",
	"double_pass",
]


def load_games(path: Path):
	opener = gzip.open if path.name.endswith(".gz") else open
	with opener(path, "rt") as fh:
		for line in fh:
			line = line.strip()
			if line:
				yield json.loads(line)


def summarize(path: Path) -> tuple[int, Counter]:
	counts: Counter = Counter()
	n = 0
	for rec in load_games(path):
		n += 1
		end = rec.get("end") or {}
		status = end.get("status") or "?"
		reason = end.get("reason") or "?"
		counts[(status, reason)] += 1
	return n, counts


def render(title: str, n: int, counts: Counter) -> str:
	lines = [f"# {title}  games={n}"]
	for reason in REASONS:
		wins = counts.get(("won", reason), 0)
		draws = counts.get(("draw", reason), 0)
		lines.append(f"{reason:22}  wins={wins}  draws={draws}  total={wins + draws}")
	known = set(REASONS)
	for (status, reason), k in sorted(counts.items()):
		if reason not in known:
			lines.append(f"other {status}/{reason}={k}")
	return "\n".join(lines) + "\n"


def main() -> None:
	root = Path(__file__).resolve().parent
	path = root / "h2h_shallow.jsonl.gz"
	if not path.exists():
		print(f"missing {path}")
		return
	n, counts = summarize(path)
	text = render("h2h_shallow.txt local-claim", n, counts)
	(root / "ends_breakdown.txt").write_text(text)
	print(text, end="")


if __name__ == "__main__":
	main()
