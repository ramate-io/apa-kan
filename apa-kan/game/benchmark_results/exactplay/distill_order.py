#!/usr/bin/env python3
"""Fit existing cheap_order integers from paired light vs 4x root dumps.

Only the knobs already on SearchConfig:

  order_vacate_center, order_vacate_overwhelmed,
  order_center_threat, order_claim_center

No new features, priors, or tactics. Train on games 1–3 of each pairing
(hold out games 4–5). Promote a TOML only after a later H2H confirm:
light sit-and-win must drop *and* 4x-vs-light gap must shrink.

  python3 distill_order.py fit \\
    --roots distill/roots_deep4x_self.jsonl.gz \\
    --out-toml distill/candidate.toml \\
    --report distill/fit_report.json
"""

from __future__ import annotations

import argparse
import json
from itertools import product
from pathlib import Path

CURRENT = {
    "order_vacate_center": 1200,
    "order_vacate_overwhelmed": 1200,
    "order_center_threat": 0,
    "order_claim_center": 0,
}

GRID = {
    "order_vacate_center": [0, 400, 800, 1200, 2000],
    "order_vacate_overwhelmed": [0, 1200],
    "order_center_threat": [0, 2000, 3500, 5000],
    "order_claim_center": [0, 2500, 5000],
}

KEYS = tuple(CURRENT)


def load_roots(path: Path) -> list[dict]:
    import gzip

    rows = []
    opener = gzip.open if path.name.endswith(".gz") else open
    with opener(path, "rt", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                rows.append(json.loads(line))
    return rows


def is_train(row: dict) -> bool:
    return int(row.get("game") or 0) <= 3


def score_move(feat: dict, w: dict[str, int]) -> int:
    return int(feat.get("base") or 0) - int(feat.get("vacate_healthy") or 0) * w[
        "order_vacate_center"
    ] - int(feat.get("vacate_overwhelmed") or 0) * w[
        "order_vacate_overwhelmed"
    ] + int(feat.get("center_threat") or 0) * w[
        "order_center_threat"
    ] + int(feat.get("claim_center") or 0) * w["order_claim_center"]


def deep_rank(row: dict, w: dict[str, int]) -> tuple[int, int, int]:
    """Return (rank of deep, n_legal, beat_light). Rank 1 is best."""
    legal = row.get("legal") or []
    deep_f = (row.get("deep") or {}).get("features") or {}
    light_f = (row.get("light") or {}).get("features") or {}
    ds = score_move(deep_f, w)
    ls = score_move(light_f, w)
    deep_kind = ((row.get("deep") or {}).get("action") or {}).get("kind")
    deep_from = ((row.get("deep") or {}).get("action") or {}).get("from")
    deep_to = ((row.get("deep") or {}).get("action") or {}).get("to")
    deep_idx = None
    for i, m in enumerate(legal):
        if m.get("kind") == deep_kind and m.get("from") == deep_from and m.get("to") == deep_to:
            deep_idx = i
            break
    if not legal:
        return 1, 0, int(ds > ls)
    scored = sorted(((score_move(m, w), i) for i, m in enumerate(legal)), reverse=True)
    order = [i for _, i in scored]
    if deep_idx is None:
        better = sum(1 for m in legal if score_move(m, w) > ds)
        rank = better + 1
    else:
        rank = order.index(deep_idx) + 1
    return rank, len(legal), int(ds > ls)


def eval_weights(rows: list[dict], w: dict[str, int]) -> dict:
    """Score *disagreements only*, with extra weight on center-touching roots."""
    n = 0
    disagree = 0
    beat_light = 0
    ranks: list[int] = []
    center_ranks: list[int] = []
    vacate_dis_ranks: list[int] = []
    threat_dis_ranks: list[int] = []
    weighted_mrr = 0.0
    weighted_n = 0.0
    vacate_chosen = 0
    threat_chosen = 0
    for row in rows:
        if not row.get("disagree"):
            continue
        rank, n_legal, beat = deep_rank(row, w)
        if n_legal == 0:
            continue
        deep_f = (row.get("deep") or {}).get("features") or {}
        n += 1
        disagree += 1
        beat_light += beat
        ranks.append(rank)
        wt = 3.0 if row.get("center_touch") else 1.0
        weighted_mrr += wt / rank
        weighted_n += wt
        if row.get("center_touch"):
            center_ranks.append(rank)
        if deep_f.get("off_center"):
            vacate_chosen += 1
            vacate_dis_ranks.append(rank)
        if deep_f.get("center_threat"):
            threat_chosen += 1
            threat_dis_ranks.append(rank)

    def mean(xs: list[int]) -> float:
        return sum(xs) / len(xs) if xs else 0.0

    mrr = sum(1.0 / r for r in ranks) / len(ranks) if ranks else 0.0
    return {
        "n": n,
        "disagree": disagree,
        "beat_light": beat_light,
        "beat_light_rate": beat_light / disagree if disagree else 0.0,
        "mean_rank": mean(ranks),
        "mrr": mrr,
        "weighted_mrr": weighted_mrr / weighted_n if weighted_n else 0.0,
        "center_mean_rank": mean(center_ranks),
        "vacate_dis_mean_rank": mean(vacate_dis_ranks),
        "threat_dis_mean_rank": mean(threat_dis_ranks),
        "vacate_chosen": vacate_chosen,
        "threat_chosen": threat_chosen,
        "top1": sum(1 for r in ranks if r == 1) / n if n else 0.0,
    }


def write_toml(path: Path, w: dict[str, int], description: str) -> None:
    lines = [
        'name = "exactplay_distill"',
        f'description = "{description}"',
        "eval_margin_scale = 200",
    ]
    for k in KEYS:
        lines.append(f"{k} = {w[k]}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def fit(args: argparse.Namespace) -> None:
    rows = load_roots(Path(args.roots))
    train = [r for r in rows if is_train(r)]
    hold = [r for r in rows if not is_train(r)]
    current_train = eval_weights(train, CURRENT)
    current_hold = eval_weights(hold, CURRENT)
    best_w = dict(CURRENT)
    best_s = current_train
    tried = 0
    for vals in product(*(GRID[k] for k in KEYS)):
        w = dict(zip(KEYS, vals))
        tried += 1
        s = eval_weights(train, w)
        # Disagreement-only, center-touch weighted. Then beat-light, then rank.
        key = (s["weighted_mrr"], s["beat_light_rate"], -s["mean_rank"])
        best_key = (best_s["weighted_mrr"], best_s["beat_light_rate"], -best_s["mean_rank"])
        if key > best_key:
            best_w = w
            best_s = s
    best_hold = eval_weights(hold, best_w)
    changed = best_w != CURRENT
    report = {
        "n_roots": len(rows),
        "n_train": len(train),
        "n_hold": len(hold),
        "tried": tried,
        "current": CURRENT,
        "current_train": current_train,
        "current_hold": current_hold,
        "best": best_w,
        "best_train": best_s,
        "best_hold": best_hold,
        "changed": changed,
        "holdout_mrr_improved": best_hold["weighted_mrr"]
        > current_hold["weighted_mrr"] + 1e-9,
    }
    out_toml = Path(args.out_toml)
    desc = (
        "Fitted cheap_order knobs from 4x-vs-light root disagreements "
        f"(vacate={best_w['order_vacate_center']}, "
        f"threat={best_w['order_center_threat']})"
    )
    write_toml(out_toml, best_w, desc)
    Path(args.report).write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    print(f"wrote {out_toml}")


def main() -> None:
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest="cmd", required=True)
    f = sub.add_parser("fit")
    f.add_argument("--roots", required=True)
    f.add_argument("--out-toml", required=True)
    f.add_argument("--report", required=True)
    args = p.parse_args()
    if args.cmd == "fit":
        fit(args)


if __name__ == "__main__":
    main()
