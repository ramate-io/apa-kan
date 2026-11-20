#!/usr/bin/env python3
"""Win/draw split plus control-cascade kind/target from per-game JSONL."""

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


def summarize(path: Path) -> tuple[int, Counter, Counter, Counter]:
	counts: Counter = Counter()
	kinds: Counter = Counter()
	targets: Counter = Counter()
	n = 0
	for rec in load_games(path):
		n += 1
		end = rec.get("end") or {}
		status = end.get("status") or "?"
		reason = end.get("reason") or "?"
		counts[(status, reason)] += 1
		if reason == "control":
			kinds[end.get("control_win_kind") or "?"] += 1
			targets[end.get("control_target")] += 1
	return n, counts, kinds, targets


def render(title: str, n: int, counts: Counter, kinds: Counter, targets: Counter) -> str:
	lines = [f"# {title}  games={n}"]
	for reason in REASONS:
		wins = counts.get(("won", reason), 0)
		draws = counts.get(("draw", reason), 0)
		lines.append(f"{reason:22}  wins={wins}  draws={draws}  total={wins + draws}")
	known = set(REASONS)
	for (status, reason), k in sorted(counts.items()):
		if reason not in known:
			lines.append(f"other {status}/{reason}={k}")
	if kinds:
		kind_bits = "  ".join(f"{k}={v}" for k, v in sorted(kinds.items()))
		lines.append(f"control_win_kind         {kind_bits}")
	if targets:
		tgt_bits = "  ".join(f"target{t}={v}" for t, v in sorted(targets.items(), key=lambda kv: (kv[0] is None, kv[0])))
		lines.append(f"control_target           {tgt_bits}")
	return "\n".join(lines) + "\n"


def main() -> None:
	root = Path(__file__).resolve().parent
	path = root / "h2h_shallow_ratchet.jsonl.gz"
	if not path.exists():
		print(f"missing {path}")
		return
	n, counts, kinds, targets = summarize(path)
	text = render("h2h_shallow_ratchet.txt control-cascade", n, counts, kinds, targets)
	(root / "ends_breakdown_ratchet.txt").write_text(text)
	print(text, end="")


if __name__ == "__main__":
	main()
