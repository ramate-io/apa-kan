#!/usr/bin/env python3
"""Survey next-move ratchet-reward JSONL.gz logs.

Reuses the exactplay center-walk / draft / braid-hit survey, then adds
ratchet counts, 1st/2nd+ reward use (move/swap/flip + ring), lockouts
right after a flip/swap, and first-ratchet leader win/comeback rate.
"""

from __future__ import annotations

import math
import statistics
import sys
from collections import Counter, defaultdict
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


def node_ring(node) -> str:
    if node == CENTER:
        return "center"
    if node in INNER:
        return "inner"
    return "outer"


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
    by_pair: dict[str, list[dict]] = defaultdict(list)
    for g in games:
        by_pair[str(g.get("pairing") or "?")].append(g)
    print("  by pairing:")
    for pairing, subset in sorted(by_pair.items()):
        w = sum(1 for g in subset if (g.get("end") or {}).get("winner") == 1)
        d = sum(
            1
            for g in subset
            if (g.get("end") or {}).get("winner") in (None, 0)
        )
        l = sum(1 for g in subset if (g.get("end") or {}).get("winner") == 2)
        nn = len(subset)
        p, lo, hi = wilson_ci(w, nn)
        pt = [g.get("turns") or 0 for g in subset]
        print(
            f"    {pairing:22s} n={nn:3d} P1 {w}-{d}-{l}  "
            f"win {p:.1%} [{lo:.1%}, {hi:.1%}]  "
            f"mean {base.mean(pt):.2f}  median {median(pt):.1f}"
        )
    by_block: dict[str, list[dict]] = defaultdict(list)
    for g in games:
        seed = int(g.get("seed") or 0)
        if 300 <= seed <= 449:
            by_block["seeds 300-449"].append(g)
        elif 10300 <= seed <= 10449:
            by_block["seeds 10300-10449"].append(g)
    if by_block:
        print("  by seed block:")
        for label, subset in sorted(by_block.items()):
            w = sum(1 for g in subset if (g.get("end") or {}).get("winner") == 1)
            d = sum(
                1
                for g in subset
                if (g.get("end") or {}).get("winner") in (None, 0)
            )
            l = sum(1 for g in subset if (g.get("end") or {}).get("winner") == 2)
            nn = len(subset)
            p, lo, hi = wilson_ci(w, nn)
            pt = [g.get("turns") or 0 for g in subset]
            print(
                f"    {label:22s} n={nn:3d} P1 {w}-{d}-{l}  "
                f"win {p:.1%} [{lo:.1%}, {hi:.1%}]  "
                f"mean {base.mean(pt):.2f}  median {median(pt):.1f}"
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
    ratchet_counts = []
    ratchet_kinds = Counter()
    reward_by_tier_kind = Counter()
    reward_rings = Counter()
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
    lockout_after_flip_swap = 0
    lockout_after_reward = Counter()
    lockout_after_extra = Counter()
    lockout_after_examples = []
    first_cascade_leader_wins = 0
    first_cascade_games = 0
    first_cascade_comebacks = 0
    cascade_examples = []

    for g in games:
        pairing = g.get("pairing") or "?"
        game_n = g.get("game")
        end = g.get("end") or {}
        winner = end.get("winner")
        reason = end.get("reason")
        fight_awarded += int(end.get("fight_gain_awarded") or 0)
        fight_mover += int(end.get("fight_gain_mover") or 0)
        fight_opp += int(end.get("fight_gain_opponent") or 0)
        rc = int(end.get("ratchet_count") or end.get("fight_gain_awarded") or 0)
        ratchet_counts.append(rc)

        first_leader = None
        first_leader_kind = None
        first_leader_turn = None
        actions = g.get("actions") or []
        for act in actions:
            tg = act.get("turn_gain")
            fg = act.get("fight_gain")
            rk = act.get("ratchet_kind")
            if rk:
                ratchet_kinds[rk] += 1
            if first_leader is None and (tg or fg):
                player = act.get("player")
                if tg == "mover" or fg == "mover":
                    first_leader = player
                elif tg == "opponent" or fg == "opponent":
                    first_leader = 2 if player == 1 else 1
                first_leader_kind = rk or ("fight" if fg else "cascade")
                first_leader_turn = act.get("turn")
            reward = bool(act.get("played_as_ratchet"))
            if not reward:
                continue
            kind = act.get("kind") or "?"
            extra_kinds[kind] += 1
            tier = act.get("ratchet_tier") or ("later" if kind == "CONVERT" else "first")
            reward_by_tier_kind[f"{tier}:{kind}"] += 1
            if kind == "SWAP":
                frm = act.get("from")
                to = act.get("to")
                piece = act.get("piece") or {}
                reward_rings[f"swap-{node_ring(to)}"] += 1
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
                        "tier": tier,
                        "from": frm,
                        "to": to,
                        "n_rings": piece.get("n_rings"),
                        "partner_owner": partner_owner,
                        "center": frm == CENTER or to == CENTER,
                        "winner": winner,
                        "turns": g.get("turns"),
                        "reason": reason,
                    }
                )
            elif kind == "CONVERT":
                node = act.get("to")
                piece = act.get("piece") or {}
                loc = node_ring(node)
                reward_rings[f"flip-{loc}"] += 1
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
                        "tier": tier,
                        "node": node,
                        "n_rings": piece.get("n_rings"),
                        "self_pop": self_pop,
                        "capture": int(act.get("capture") or 0),
                        "center": node == CENTER,
                        "winner": winner,
                        "turns": g.get("turns"),
                        "reason": reason,
                    }
                )
            else:
                reward_rings[f"{kind.lower()}-n/a"] += 1
        if reason == "locked_out" and actions:
            last = actions[-1]
            last_kind = last.get("kind")
            ratchet_last = bool(last.get("played_as_ratchet"))
            extra_last = bool(last.get("played_as_extra"))
            if ratchet_last:
                lockout_after_reward[last_kind or "?"] += 1
            if extra_last:
                lockout_after_extra[last_kind or "?"] += 1
            if last_kind in ("CONVERT", "SWAP"):
                lockout_after_flip_swap += 1
            reward_last = ratchet_last or extra_last
            if reward_last or last_kind in ("CONVERT", "SWAP"):
                lockout_after_examples.append(
                    {
                        "pairing": pairing,
                        "game": game_n,
                        "seed": g.get("seed"),
                        "verb": last_kind,
                        "reward": reward_last,
                        "turn": last.get("turn"),
                        "player": last.get("player"),
                        "winner": winner,
                        "turns": g.get("turns"),
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
                    "reason": reason,
                    "won": won,
                }
            )

    print("\n# ratchet / reward use")
    avg_r = (sum(ratchet_counts) / n) if n else 0.0
    print(
        f"  ratchet_count mean={avg_r:.2f} median={median([float(x) for x in ratchet_counts]):.1f}  "
        f"games_with_1+={sum(1 for x in ratchet_counts if x >= 1)}  "
        f"games_with_2+={sum(1 for x in ratchet_counts if x >= 2)}  "
        f"kinds {dict(ratchet_kinds)}"
    )
    print(
        f"  fight_gain awarded={fight_awarded} avg={fight_awarded / n if n else 0:.2f}  "
        f"mover={fight_mover} opponent={fight_opp}"
    )
    extra_n = sum(extra_kinds.values())
    print(f"  reward actions {extra_n}  kinds {dict(extra_kinds)}")
    print(f"  by tier:kind {dict(reward_by_tier_kind)}")
    print(f"  target rings {dict(reward_rings)}")
    print(
        f"  reward SWAP center={extra_swaps_center} control={extra_swaps_control} "
        f"outer={extra_swaps_outer} own={extra_swaps_own} opp={extra_swaps_opp}  "
        f"multi_ring_initiator={extra_swaps_multi}"
    )
    print(
        f"  reward CONVERT center={extra_converts_center} inner={extra_converts_inner} "
        f"outer={extra_converts_outer} self_pop={extra_converts_self_pop}  "
        f"rings={dict(extra_converts_rings)}"
    )
    lockouts = reasons.get("locked_out", 0)
    reward_lock_n = sum(lockout_after_reward.values())
    extra_lock_n = sum(lockout_after_extra.values())
    print(
        f"  lockouts after flip/swap {lockout_after_flip_swap}/{lockouts} "
        f"({base.pct(lockout_after_flip_swap, lockouts)})"
    )
    print(
        f"  lockouts right after a ratchet reward {reward_lock_n}/{lockouts} "
        f"({base.pct(reward_lock_n, lockouts)})  by verb {dict(lockout_after_reward)}"
    )
    print(
        f"  lockouts right after a cascade extra {extra_lock_n}/{lockouts} "
        f"({base.pct(extra_lock_n, lockouts)})  by verb {dict(lockout_after_extra)}"
    )
    print(
        f"  leader after first ratchet: games={first_cascade_games}  "
        f"leader_wins={first_cascade_leader_wins} "
        f"({base.pct(first_cascade_leader_wins, first_cascade_games)})  "
        f"comebacks={first_cascade_comebacks} "
        f"({base.pct(first_cascade_comebacks, first_cascade_games)})"
    )
    pair_come: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    for e in cascade_examples:
        slot = pair_come[str(e["pairing"] or "?")]
        slot[0] += 1
        if not e["won"] and e["winner"] not in (None, 0):
            slot[1] += 1
    if pair_come:
        print("  comebacks by pairing:")
        for pairing, (ng, nc) in sorted(pair_come.items()):
            print(f"    {pairing:22s} {nc}/{ng} ({base.pct(nc, ng)})")

    print("\n# lockouts immediately after a reward flip/swap")
    for ex in lockout_after_examples[:8]:
        print(
            f"  {ex['pairing']} game {ex['game']} t{ex['turn']} P{ex['player']} "
            f"{ex['verb']} reward={ex['reward']} end=locked_out "
            f"W={ex['winner']} turns={ex['turns']}"
        )
    if not lockout_after_examples:
        print("  (none)")

    print("\n# reward SWAP examples")
    for ex in extra_examples[:12]:
        print(
            f"  {ex['pairing']} game {ex['game']} t{ex['turn']} P{ex['player']} "
            f"tier={ex['tier']} SWAP {ex['from']}<->{ex['to']} rings={ex['n_rings']} "
            f"partner=P{ex['partner_owner']} center={ex['center']} "
            f"end={ex['reason']} W={ex['winner']} turns={ex['turns']}"
        )
    if not extra_examples:
        print("  (none)")

    print("\n# reward CONVERT examples")
    for ex in extra_convert_examples[:16]:
        loc = "center" if ex["center"] else f"node {ex['node']}"
        print(
            f"  {ex['pairing']} game {ex['game']} t{ex['turn']} P{ex['player']} "
            f"tier={ex['tier']} CONVERT {loc} rings={ex['n_rings']} "
            f"self_pop={ex['self_pop']} opp_caps={ex['capture']} "
            f"end={ex['reason']} W={ex['winner']} turns={ex['turns']}"
        )
    if not extra_convert_examples:
        print("  (none)")

    print("\n# first-ratchet leader examples (losses = comebacks)")
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
