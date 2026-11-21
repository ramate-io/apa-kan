#!/usr/bin/env python3
"""Survey cascade-winner extra swap / center-mismatch JSONL.gz logs.

Reuses the exactplay center-walk / draft / braid-hit survey, then adds
extra-turn usage, fight-gain, length distribution, and first-cascade
leader win rate.
"""

from __future__ import annotations

import math
import statistics
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "exactplay"))
import survey as base  # noqa: E402

CENTER = base.CENTER
INNER = base.INNER
CONTROL = base.CONTROL


def median(xs: list[float]) -> float:
    return float(statistics.median(xs)) if xs else 0.0


def wilson_ci(k: int, n: int, z: float = 1.96) -> tuple[float, float, float]:
    """Wilson 95% interval for k successes in n trials. Returns (p, lo, hi)."""
    if n <= 0:
        return 0.0, 0.0, 0.0
    p = k / n
    z2 = z * z
    den = 1.0 + z2 / n
    centre = (p + z2 / (2.0 * n)) / den
    margin = z * math.sqrt((p * (1.0 - p) + z2 / (4.0 * n)) / n) / den
    return p, max(0.0, centre - margin), min(1.0, centre + margin)


def extra_survey(games: list[dict]) -> None:
    n = len(games)
    turns = [g.get("turns") or 0 for g in games]
    reasons = Counter((g.get("end") or {}).get("reason") for g in games)
    winners = Counter((g.get("end") or {}).get("winner") for g in games)
    p1_w = winners.get(1, 0)
    p1_d = winners.get(None, 0) + winners.get(0, 0)
    p1_l = winners.get(2, 0)
    win_p, win_lo, win_hi = wilson_ci(p1_w, n)
    print("\n# length / seat")
    print(
        f"  P1 {p1_w}-{p1_d}-{p1_l}  games={n}  "
        f"P1 win {win_p:.1%} [{win_lo:.1%}, {win_hi:.1%}]  "
        f"mean {base.mean(turns):.2f}  median {median(turns):.1f}  "
        f"p90 {sorted(turns)[int(0.9 * (n - 1))] if n else 0}"
    )
    buckets = Counter()
    for t in turns:
        if t <= 20:
            buckets["<=20"] += 1
        elif t <= 40:
            buckets["21-40"] += 1
        elif t <= 60:
            buckets["41-60"] += 1
        elif t <= 80:
            buckets["61-80"] += 1
        elif t <= 120:
            buckets["81-120"] += 1
        elif t < 200:
            buckets["121-199"] += 1
        else:
            buckets["200"] += 1
    print(f"  dist {dict(buckets)}  turn_limit={reasons.get('turn_limit', 0)}  "
          f"lockout={reasons.get('locked_out', 0)}")

    fight_awarded = 0
    fight_mover = 0
    fight_opp = 0
    extra_kinds = Counter()
    extra_swaps_center = 0
    extra_swaps_own = 0
    extra_swaps_opp = 0
    extra_swaps_multi = 0
    extra_swaps_control = 0
    extra_swaps_outer = 0
    extra_examples = []
    extra_converts_center = 0
    extra_converts_inner = 0
    extra_converts_outer = 0
    extra_converts_self_pop = 0
    extra_converts_rings = Counter()
    extra_convert_examples = []
    first_cascade_leader_wins = 0
    first_cascade_games = 0
    first_cascade_comebacks = 0
    cascade_examples = []

    for gi, g in enumerate(games):
        pairing = g.get("pairing") or "?"
        game_n = g.get("game")
        end = g.get("end") or {}
        winner = end.get("winner")
        fight_awarded += int(end.get("fight_gain_awarded") or 0)
        fight_mover += int(end.get("fight_gain_mover") or 0)
        fight_opp += int(end.get("fight_gain_opponent") or 0)

        first_leader = None
        first_leader_kind = None
        first_leader_turn = None
        for act in g.get("actions") or []:
            tg = act.get("turn_gain")
            fg = act.get("fight_gain")
            if first_leader is None and (tg or fg):
                player = act.get("player")
                if tg == "mover" or fg == "mover":
                    first_leader = player
                elif tg == "opponent" or fg == "opponent":
                    first_leader = 2 if player == 1 else 1
                first_leader_kind = "fight" if fg else "cascade"
                first_leader_turn = act.get("turn")
            if act.get("played_as_extra"):
                kind = act.get("kind") or "?"
                extra_kinds[kind] += 1
                if kind == "SWAP":
                    frm = act.get("from")
                    to = act.get("to")
                    piece = act.get("piece") or {}
                    if frm == CENTER or to == CENTER:
                        extra_swaps_center += 1
                    partner_owner = act.get("partner_owner")
                    if partner_owner == act.get("player"):
                        extra_swaps_own += 1
                    elif partner_owner is not None:
                        extra_swaps_opp += 1
                    if (piece.get("n_rings") or 0) != 1:
                        extra_swaps_multi += 1
                    if frm in CONTROL or to in CONTROL:
                        extra_swaps_control += 1
                    else:
                        extra_swaps_outer += 1
                    extra_examples.append(
                        {
                            "pairing": pairing,
                            "game": game_n,
                            "turn": act.get("turn"),
                            "player": act.get("player"),
                            "from": frm,
                            "to": to,
                            "n_rings": piece.get("n_rings"),
                            "partner_owner": partner_owner,
                            "center": frm == CENTER or to == CENTER,
                            "winner": winner,
                            "turns": g.get("turns"),
                            "reason": end.get("reason"),
                        }
                    )
                elif kind == "CONVERT":
                    node = act.get("to")
                    piece = act.get("piece") or {}
                    if node == CENTER:
                        extra_converts_center += 1
                    elif node in INNER:
                        extra_converts_inner += 1
                    else:
                        extra_converts_outer += 1
                    extra_converts_rings[piece.get("n_rings") or 0] += 1
                    self_pop = int(act.get("self_capture") or 0) > 0
                    if self_pop:
                        extra_converts_self_pop += 1
                    extra_convert_examples.append(
                        {
                            "pairing": pairing,
                            "game": game_n,
                            "turn": act.get("turn"),
                            "player": act.get("player"),
                            "node": node,
                            "n_rings": piece.get("n_rings"),
                            "self_pop": self_pop,
                            "capture": int(act.get("capture") or 0),
                            "center": node == CENTER,
                            "winner": winner,
                            "turns": g.get("turns"),
                            "reason": end.get("reason"),
                        }
                    )
        if first_leader is not None:
            first_cascade_games += 1
            won = winner == first_leader
            if won:
                first_cascade_leader_wins += 1
            elif winner not in (None, 0) and winner != first_leader:
                first_cascade_comebacks += 1
            cascade_examples.append(
                {
                    "pairing": pairing,
                    "game": game_n,
                    "turn": first_leader_turn,
                    "kind": first_leader_kind,
                    "leader": first_leader,
                    "winner": winner,
                    "turns": g.get("turns"),
                    "reason": end.get("reason"),
                    "won": won,
                }
            )

    print("\n# fight_gain / extra-turn use")
    print(
        f"  fight_gain awarded={fight_awarded} avg={fight_awarded / n if n else 0:.2f}  "
        f"mover={fight_mover} opponent={fight_opp}"
    )
    extra_n = sum(extra_kinds.values())
    print(f"  extra actions {extra_n}  kinds {dict(extra_kinds)}")
    print(
        f"  extra SWAP center={extra_swaps_center} control={extra_swaps_control} "
        f"outer={extra_swaps_outer} own={extra_swaps_own} opp={extra_swaps_opp}  "
        f"multi_ring_initiator={extra_swaps_multi}"
    )
    print(
        f"  extra CONVERT center={extra_converts_center} inner={extra_converts_inner} "
        f"outer={extra_converts_outer} self_pop={extra_converts_self_pop}  "
        f"rings={dict(extra_converts_rings)}"
    )
    print(
        f"  leader after first cascade/fight: games={first_cascade_games}  "
        f"leader_wins={first_cascade_leader_wins} "
        f"({base.pct(first_cascade_leader_wins, first_cascade_games)})  "
        f"comebacks={first_cascade_comebacks} "
        f"({base.pct(first_cascade_comebacks, first_cascade_games)})"
    )

    print("\n# extra SWAP examples")
    for ex in extra_examples[:12]:
        print(
            f"  {ex['pairing']} game {ex['game']} t{ex['turn']} P{ex['player']} "
            f"SWAP {ex['from']}<->{ex['to']} rings={ex['n_rings']} "
            f"partner=P{ex['partner_owner']} center={ex['center']} "
            f"end={ex['reason']} W={ex['winner']} turns={ex['turns']}"
        )
    if not extra_examples:
        print("  (none)")

    print("\n# extra CONVERT examples")
    for ex in extra_convert_examples[:16]:
        loc = "center" if ex["center"] else f"node {ex['node']}"
        print(
            f"  {ex['pairing']} game {ex['game']} t{ex['turn']} P{ex['player']} "
            f"CONVERT {loc} rings={ex['n_rings']} self_pop={ex['self_pop']} "
            f"opp_caps={ex['capture']} "
            f"end={ex['reason']} W={ex['winner']} turns={ex['turns']}"
        )
    if not extra_convert_examples:
        print("  (none)")

    print("\n# first-cascade leader examples (losses = comebacks)")
    losses = [e for e in cascade_examples if not e["won"] and e["winner"] not in (None, 0)]
    wins = [e for e in cascade_examples if e["won"]]
    for label, subset in (("leader_lost", losses[:6]), ("leader_won", wins[:4])):
        print(f"  {label}:")
        if not subset:
            print("    (none)")
        for e in subset:
            print(
                f"    {e['pairing']} game {e['game']} t{e['turn']} {e['kind']} "
                f"leader=P{e['leader']} winner=P{e['winner']} "
                f"end={e['reason']} turns={e['turns']}"
            )


def summarize_one(path: Path) -> None:
    games = base.load_games(path)
    print(f"===== {path} =====")
    base.summarize(path)
    extra_survey(games)
    print()


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: survey.py <games.jsonl.gz> [...]", file=sys.stderr)
        sys.exit(2)
    for arg in sys.argv[1:]:
        summarize_one(Path(arg))


if __name__ == "__main__":
    main()
