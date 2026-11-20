#!/usr/bin/env python3
"""Counterplay survey for same-deck JSONL.gz logs.

A first center *take* counts only if the landing piece is still on the center
after that action's captures resolve. Inner-ring node ids are the unique
neighbors of the origin on the 8-petal rosette.
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
    opener = gzip.open if path.suffix == ".gz" or path.name.endswith(".jsonl.gz") else open
    games = []
    with opener(path, "rt", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                games.append(json.loads(line))
    return games


def occupant_at_end(game: dict) -> tuple[int | None, int | None]:
    final = game.get("final_state") or {}
    for owner_key, owner in (("p1_board", 1), ("p2_board", 2)):
        for piece in final.get(owner_key) or []:
            if piece.get("node") == CENTER:
                return owner, piece.get("id")
    return None, None


def walk(game: dict) -> dict:
    occ_owner: dict[int, int] = {}
    occ_id: dict[int, int] = {}
    first_owner = None
    first_id = None
    first_turn = None
    first_kind = None
    holders = 0
    vacates = 0
    center_opp_caps = 0
    center_self_caps = 0
    center_mutual = 0
    center_claims = 0
    holder_caps_inner = 0
    inner_caps_holder = 0
    zero_budget_actions = 0
    n_actions = 0
    budget = {1: None, 2: None}
    opening = game.get("opening") or {}
    if "p1_tokens" in opening:
        budget[1] = opening.get("p1_tokens")
    if "p2_tokens" in opening:
        budget[2] = opening.get("p2_tokens")

    for act in game.get("actions") or []:
        kind = act.get("kind")
        player = act.get("player")
        to = act.get("to")
        frm = act.get("from")
        piece = act.get("piece") or {}
        pid = piece.get("id")
        turn = act.get("turn")
        captures = act.get("captures") or []
        captured_nodes = {c.get("node") for c in captures}
        n_actions += 1

        before = budget.get(player)
        if before == 0:
            zero_budget_actions += 1
        if act.get("budget") is not None:
            budget[player] = act.get("budget")

        holder = occ_owner.get(CENTER)

        if kind == "CLAIM" and to == CENTER:
            center_claims += 1

        for cap in captures:
            node = cap.get("node")
            owner = cap.get("owner")
            if node == CENTER:
                if owner == player:
                    center_self_caps += 1
                else:
                    center_opp_caps += 1
                    actor_lost = any(c.get("owner") == player for c in captures)
                    if actor_lost:
                        center_mutual += 1
            if holder is not None and node in INNER:
                if owner != holder:
                    holder_caps_inner += 1
                else:
                    inner_caps_holder += 1

        if kind == "MOVE" and frm is not None:
            occ_owner.pop(frm, None)
            occ_id.pop(frm, None)
            if frm == CENTER:
                vacates += 1

        for cap in captures:
            node = cap.get("node")
            occ_owner.pop(node, None)
            occ_id.pop(node, None)

        entered = kind in ("ADD", "MOVE") and to is not None
        if entered and to not in captured_nodes:
            occ_owner[to] = player
            occ_id[to] = pid
            if to == CENTER:
                holders += 1
                if first_owner is None:
                    first_owner = player
                    first_id = pid
                    first_turn = turn
                    first_kind = kind

    end_owner, end_id = occupant_at_end(game)
    still_first = (
        first_id is not None
        and end_id is not None
        and first_id == end_id
        and end_owner == first_owner
    )
    never_left = still_first and holders == 1
    winner = (game.get("end") or {}).get("winner")
    first_side_won = first_owner is not None and winner == first_owner
    return {
        "game": game.get("game"),
        "pairing": game.get("pairing"),
        "matchup": game.get("deck_matchup"),
        "turns": game.get("turns"),
        "winner": winner,
        "reason": (game.get("end") or {}).get("reason"),
        "first_owner": first_owner,
        "first_id": first_id,
        "first_turn": first_turn,
        "first_kind": first_kind,
        "end_owner": end_owner,
        "end_id": end_id,
        "still_first": still_first,
        "never_left": never_left,
        "first_side_won": first_side_won,
        "holders": holders,
        "vacates": vacates,
        "center_opp_caps": center_opp_caps,
        "center_self_caps": center_self_caps,
        "center_mutual": center_mutual,
        "center_claims": center_claims,
        "holder_caps_inner": holder_caps_inner,
        "inner_caps_holder": inner_caps_holder,
        "zero_budget_actions": zero_budget_actions,
        "n_actions": n_actions,
        "p1_search": game.get("p1_search", ""),
        "p2_search": game.get("p2_search", ""),
        "claims_total": sum(1 for a in game.get("actions") or [] if a.get("kind") == "CLAIM"),
    }


def tile_kind(tile: dict) -> tuple:
    rings = tuple(
        (r.get("c"), r.get("z"), r.get("b"), r.get("e")) for r in (tile.get("rings") or [])
    )
    return (
        tile.get("n_rings"),
        tile.get("dots"),
        tile.get("capture"),
        tile.get("emblem"),
        rings,
    )


def same_deck_stats(games: list[dict]) -> tuple[int, int, int]:
    with_setup = 0
    identical = 0
    mismatched = 0
    for g in games:
        setup = g.get("setup") or {}
        p1 = setup.get("p1_deck") or []
        p2 = setup.get("p2_deck") or []
        if not p1 and not p2:
            continue
        with_setup += 1
        k1 = [tile_kind(t) for t in p1]
        k2 = [tile_kind(t) for t in p2]
        if len(p1) == 27 and len(p2) == 27 and k1 == k2:
            identical += 1
        else:
            mismatched += 1
    return with_setup, identical, mismatched


def pct(n: int, d: int) -> str:
    if not d:
        return "—"
    return f"{n / d * 100:.1f}%"


def wdl(rows: list[dict]) -> str:
    w = sum(1 for r in rows if r["winner"] == 1)
    d = sum(1 for r in rows if r["winner"] is None or r["winner"] == 0)
    l = sum(1 for r in rows if r["winner"] == 2)
    return f"{w}-{d}-{l}"


def mean(xs: list[float]) -> float:
    return sum(xs) / len(xs) if xs else 0.0


def summarize(path: Path) -> dict:
    games = load_games(path)
    rows = [walk(g) for g in games]
    n = len(rows)
    with_setup, identical_27, mismatched = same_deck_stats(games)
    took = [r for r in rows if r["first_owner"] is not None]
    still = [r for r in took if r["still_first"]]
    never_left = [r for r in took if r["never_left"]]
    first_wins = [r for r in took if r["first_side_won"]]
    reasons = Counter(r["reason"] for r in rows)
    winners = Counter(r["winner"] for r in rows)
    kinds = Counter(r["first_kind"] for r in took)
    mean_turns = mean([r["turns"] or 0 for r in rows])
    mean_first_turn = mean([r["first_turn"] or 0 for r in took])
    opp_caps = mean([r["center_opp_caps"] for r in rows])
    mutual = mean([r["center_mutual"] for r in rows])
    mutual_share = (
        sum(r["center_mutual"] for r in rows) / sum(r["center_opp_caps"] for r in rows)
        if sum(r["center_opp_caps"] for r in rows)
        else 0.0
    )
    h_inner = mean([r["holder_caps_inner"] for r in rows])
    i_holder = mean([r["inner_caps_holder"] for r in rows])
    zb = sum(r["zero_budget_actions"] for r in rows)
    na = sum(r["n_actions"] for r in rows)
    zb_share = zb / na if na else 0.0

    print(f"file {path}")
    print(f"games {n}  mean_turns {mean_turns:.2f}  P1 {wdl(rows)}")
    print(
        f"same_deck_27 identical={identical_27}/{with_setup} mismatched={mismatched}"
    )
    print(f"winners {dict(winners)}  reasons {dict(reasons)}")
    print(
        f"first_center_take {len(took)}/{n}  mean_turn {mean_first_turn:.2f}  how {dict(kinds)}"
    )
    print(
        f"never_left {len(never_left)}/{len(took)} ({pct(len(never_left), len(took))})  "
        f"still_on_center {len(still)}/{len(took)} ({pct(len(still), len(took))})"
    )
    print(
        f"first_taker_side_won {len(first_wins)}/{len(took)} ({pct(len(first_wins), len(took))})"
    )
    print(
        f"center_opp_caps/game {opp_caps:.2f}  mutual_trades/game {mutual:.2f}  "
        f"mutual_share {mutual_share * 100:.1f}%"
    )
    print(
        f"while_held inner caps: holder→opp {h_inner:.2f}  opp→holder {i_holder:.2f}"
    )
    print(f"zero_budget_actions {zb}/{na} ({zb_share * 100:.1f}%)")

    by_pair: dict[str, list[dict]] = defaultdict(list)
    for r in rows:
        by_pair[str(r["pairing"] or "?")].append(r)
    print("\n# by pairing / seat")
    for pairing, subset in sorted(by_pair.items()):
        stook = [r for r in subset if r["first_owner"] is not None]
        snever = [r for r in stook if r["never_left"]]
        swins = [r for r in stook if r["first_side_won"]]
        print(
            f"  {pairing:22s} n={len(subset):3d} P1 {wdl(subset):8s}  "
            f"turns {mean([r['turns'] or 0 for r in subset]):5.1f}  "
            f"never_left {pct(len(snever), len(stook))}  "
            f"first_wins {pct(len(swins), len(stook))}  "
            f"opp_cap {mean([r['center_opp_caps'] for r in subset]):.2f}"
        )

    def show(label: str, subset: list[dict], k: int = 5) -> None:
        print(f"\n# examples {label}")
        for r in subset[:k]:
            print(
                f"  game={r['game']} {r['pairing']} turns={r['turns']} "
                f"end={r['reason']} W={r['winner']} first=P{r['first_owner']}@t{r['first_turn']} "
                f"{r['first_kind']} never_left={r['never_left']} holders={r['holders']} "
                f"vacates={r['vacates']} opp_caps={r['center_opp_caps']} mutual={r['center_mutual']} "
                f"h→inner={r['holder_caps_inner']} inner→h={r['inner_caps_holder']}"
            )

    displaced_other_won = [
        r for r in took if not r["still_first"] and not r["first_side_won"]
    ]
    fight = sorted(rows, key=lambda r: -(r["holders"] or 0) - (r["center_opp_caps"] or 0))
    show("never_left_and_win", [r for r in never_left if r["first_side_won"]])
    show("displaced_and_other_side_won", displaced_other_won)
    show("center_opp_caps", [r for r in rows if r["center_opp_caps"] > 0])
    show("many_holders", fight)

    print("\n# qualitative")
    add_takes = sum(1 for r in took if r["first_kind"] == "ADD")
    move_takes = sum(1 for r in took if r["first_kind"] == "MOVE")
    contested = [
        r
        for r in took
        if (r["holders"] or 0) > 1 or (r["center_opp_caps"] or 0) > 0 or (r["vacates"] or 0) > 0
    ]
    sit_wins = [r for r in never_left if r["first_side_won"]]
    print(
        f"first_take how ADD {add_takes}/{len(took)} MOVE {move_takes}/{len(took)}  "
        f"contested_center {len(contested)}/{len(took)} ({pct(len(contested), len(took))})"
    )
    print(
        f"sit_and_win {len(sit_wins)}/{len(took)} ({pct(len(sit_wins), len(took))})  "
        f"nonholder_inner_caps/game {i_holder:.2f}  holder_inner_caps/game {h_inner:.2f}"
    )
    print(
        f"claims/game {mean([r['claims_total'] for r in rows]):.2f}  "
        f"center_claims/game {mean([r['center_claims'] for r in rows]):.2f}  "
        f"vacates/game {mean([r['vacates'] for r in rows]):.2f}"
    )

    singles_survey(games, path)

    return {
        "file": str(path),
        "n": n,
        "p1": wdl(rows),
        "mean_turns": mean_turns,
        "reasons": dict(reasons),
        "never_left": len(never_left),
        "took": len(took),
        "first_wins": len(first_wins),
        "mean_first_turn": mean_first_turn,
        "how": dict(kinds),
        "opp_caps": opp_caps,
        "mutual": mutual,
        "mutual_share": mutual_share,
        "holder_caps_inner": h_inner,
        "inner_caps_holder": i_holder,
        "zb_share": zb_share,
        "contested": len(contested),
        "sit_and_win": len(sit_wins),
    }


CONTROL = INNER | {CENTER}


def _deck_tiles(game: dict) -> dict[int, dict]:
    tiles: dict[int, dict] = {}
    setup = game.get("setup") or {}
    for key in ("p1_deck", "p2_deck"):
        for tile in setup.get(key) or []:
            tid = tile.get("id")
            if tid is not None:
                tiles[tid] = tile
    return tiles


def walk_ring_lives(game: dict) -> list[dict]:
    """Follow every placed piece: ADD, later MOVEs, captures, end occupancy."""
    tiles = _deck_tiles(game)
    lives: dict[int, dict] = {}
    occ_id: dict[int, int] = {}
    final_ids = set()
    final = game.get("final_state") or {}
    for key in ("p1_board", "p2_board"):
        for piece in final.get(key) or []:
            if piece.get("id") is not None:
                final_ids.add(piece["id"])

    for act in game.get("actions") or []:
        kind = act.get("kind")
        piece = act.get("piece") or {}
        pid = piece.get("id")
        to = act.get("to")
        frm = act.get("from")
        captures = act.get("captures") or []
        n_rings = piece.get("n_rings")
        if n_rings is None and pid in tiles:
            n_rings = tiles[pid].get("n_rings")
        opp_caps = sum(1 for c in captures if c.get("owner") != act.get("player"))
        self_caps = sum(1 for c in captures if c.get("owner") == act.get("player"))

        if kind == "MOVE" and frm is not None:
            occ_id.pop(frm, None)

        captured_nodes = {c.get("node") for c in captures}
        for cap in captures:
            node = cap.get("node")
            cap_id = occ_id.pop(node, None)
            if cap_id is not None and cap_id in lives:
                lives[cap_id]["captured"] = True
                lives[cap_id]["captured_owner"] = cap.get("owner")
                lives[cap_id]["captured_never_moved"] = lives[cap_id]["moves"] == 0
                lives[cap_id]["captured_on_control"] = node in CONTROL

        if kind in ("ADD", "MOVE") and pid is not None and to is not None:
            if pid not in lives:
                lives[pid] = {
                    "id": pid,
                    "n_rings": n_rings,
                    "normal_rings": piece.get("normal_rings"),
                    "zag_rings": piece.get("zag_rings"),
                    "braid_rings": piece.get("braid_rings") or 0,
                    "bead_rings": piece.get("bead_rings") or 0,
                    "owner": act.get("player"),
                    "placed_kind": kind,
                    "placed_turn": act.get("turn"),
                    "placed_node": to,
                    "moves": 0,
                    "captures_as_actor": 0,
                    "opp_caps_as_actor": 0,
                    "self_caps_as_actor": 0,
                    "healed": 0,
                    "control_visits": 0,
                    "captured": False,
                    "captured_never_moved": False,
                    "captured_on_control": False,
                    "captured_owner": None,
                }
            elif kind == "MOVE":
                lives[pid]["moves"] += 1
            if to in CONTROL:
                lives[pid]["control_visits"] += 1
            lives[pid]["captures_as_actor"] += len(captures)
            lives[pid]["opp_caps_as_actor"] += opp_caps
            lives[pid]["self_caps_as_actor"] += self_caps
            lives[pid]["healed"] += int(act.get("healed") or 0)
            if to not in captured_nodes:
                occ_id[to] = pid

    for life in lives.values():
        life["survived"] = (not life["captured"]) and life["id"] in final_ids
        life["spend_once"] = life["moves"] == 0
    return list(lives.values())


def singles_survey(games: list[dict], path: Path) -> None:
    lives = []
    for g in games:
        lives.extend(walk_ring_lives(g))
    print("\n# ring-count lives (placed pieces)")
    for n_rings, label in ((1, "singles"), (2, "doubles"), (3, "triples")):
        group = [p for p in lives if p.get("n_rings") == n_rings]
        placed = len(group)
        if not placed:
            print(f"  {label}: none placed")
            continue
        moved = sum(1 for p in group if p["moves"] > 0)
        spend = sum(1 for p in group if p["spend_once"])
        captured = sum(1 for p in group if p["captured"])
        cap_never = sum(1 for p in group if p["captured_never_moved"])
        survived = sum(1 for p in group if p["survived"])
        actor_cap = sum(1 for p in group if p["opp_caps_as_actor"] > 0)
        healed = sum(1 for p in group if p["healed"] > 0)
        control = sum(1 for p in group if p["control_visits"] > 0)
        mean_moves = mean([p["moves"] for p in group])
        print(
            f"  {label}: placed {placed}  moved_later {moved} ({pct(moved, placed)})  "
            f"never_moved {spend} ({pct(spend, placed)})  mean_moves {mean_moves:.2f}"
        )
        print(
            f"           captured {captured} ({pct(captured, placed)})  "
            f"captured_without_move {cap_never} ({pct(cap_never, placed)})  "
            f"survived {survived} ({pct(survived, placed)})"
        )
        print(
            f"           actor_opp_cap {actor_cap} ({pct(actor_cap, placed)})  "
            f"healed_someone {healed} ({pct(healed, placed)})  "
            f"touched_control {control} ({pct(control, placed)})"
        )
        kinds = Counter()
        for p in group:
            if (p.get("braid_rings") or 0) > 0:
                kinds["braid"] += 1
            elif (p.get("zag_rings") or 0) > 0:
                kinds["zag"] += 1
            elif (p.get("bead_rings") or 0) > 0:
                kinds["bead"] += 1
            else:
                kinds["normal"] += 1
        if n_rings == 1:
            print(f"           composition {dict(kinds)}")
            never = [p for p in group if p["spend_once"]]
            print(
                f"           never-moved among singles: "
                f"actor_opp_cap {sum(1 for p in never if p['opp_caps_as_actor'] > 0)}  "
                f"healed {sum(1 for p in never if p['healed'] > 0)}  "
                f"control {sum(1 for p in never if p['control_visits'] > 0)}  "
                f"survived {sum(1 for p in never if p['survived'])}"
            )


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: survey.py <games.jsonl.gz> [...]", file=sys.stderr)
        sys.exit(2)
    for arg in sys.argv[1:]:
        summarize(Path(arg))
        print()


if __name__ == "__main__":
    main()
