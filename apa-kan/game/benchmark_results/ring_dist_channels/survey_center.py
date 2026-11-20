#!/usr/bin/env python3
"""Survey first-center-taker persistence and counterplay from a games JSONL.gz."""

from __future__ import annotations

import gzip
import json
import sys
from collections import Counter
from pathlib import Path

CENTER = 0


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
    """Return (owner 1|2, tile_id) on the center at the final position, if any."""
    final = game.get("final_state") or {}
    for owner_key, owner in (("p1_board", 1), ("p2_board", 2)):
        for piece in final.get(owner_key) or []:
            if piece.get("node") == CENTER:
                return owner, piece.get("id")
    return None, None


def walk_center(game: dict) -> dict:
    first_owner = None
    first_id = None
    first_turn = None
    first_kind = None
    occupant_owner = None
    occupant_id = None
    holders = 0
    vacates = 0
    center_caps = 0
    center_self_caps = 0
    center_claims = 0
    adjacent_setups = 0
    pending_adj: dict[int, int] = {}

    for act in game.get("actions") or []:
        kind = act.get("kind")
        player = act.get("player")
        to = act.get("to")
        frm = act.get("from")
        piece = act.get("piece") or {}
        pid = piece.get("id")
        turn = act.get("turn")

        if kind in ("ADD", "MOVE") and to is not None and to != CENTER:
            if occupant_owner is not None and occupant_owner != player:
                pending_adj[player] = turn

        if kind == "CLAIM" and to == CENTER:
            center_claims += 1

        for cap in act.get("captures") or []:
            if cap.get("node") == CENTER:
                if cap.get("owner") == player:
                    center_self_caps += 1
                else:
                    center_caps += 1

        left_center = kind == "MOVE" and frm == CENTER
        entered_center = kind in ("ADD", "MOVE") and to == CENTER
        claimed_center = kind == "CLAIM" and to == CENTER

        if left_center:
            vacates += 1
            occupant_owner = None
            occupant_id = None

        captured_occupant = False
        for cap in act.get("captures") or []:
            if cap.get("node") == CENTER:
                captured_occupant = True
        if captured_occupant and not entered_center:
            occupant_owner = None
            occupant_id = None

        if entered_center or (claimed_center and kind == "CLAIM"):
            if claimed_center and not entered_center:
                occupant_owner = None
                occupant_id = None
            else:
                occupant_owner = player
                occupant_id = pid
                holders += 1
                if first_owner is None:
                    first_owner = player
                    first_id = pid
                    first_turn = turn
                    first_kind = kind
                    if player in pending_adj and first_turn is not None:
                        if first_turn - pending_adj[player] >= 1:
                            adjacent_setups += 1

    end_owner, end_id = occupant_at_end(game)
    still_first = (
        first_id is not None and end_id is not None and first_id == end_id and end_owner == first_owner
    )
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
        "first_side_won": first_side_won,
        "holders": holders,
        "vacates": vacates,
        "center_caps": center_caps,
        "center_self_caps": center_self_caps,
        "center_claims": center_claims,
        "adjacent_setups": adjacent_setups,
        "claims_total": sum(1 for a in game.get("actions") or [] if a.get("kind") == "CLAIM"),
        "p1_search": game.get("p1_search", ""),
        "p2_search": game.get("p2_search", ""),
    }


def summarize(path: Path) -> None:
    games = load_games(path)
    rows = [walk_center(g) for g in games]
    n = len(rows)
    took = [r for r in rows if r["first_owner"] is not None]
    still = [r for r in took if r["still_first"]]
    never_left = [r for r in still if r["holders"] == 1]
    first_wins = [r for r in took if r["first_side_won"]]
    still_and_win = [r for r in still if r["first_side_won"]]
    displaced = [r for r in took if not r["still_first"]]
    displaced_first_still_wins = [r for r in displaced if r["first_side_won"]]
    reasons = Counter(r["reason"] for r in rows)
    winners = Counter(r["winner"] for r in rows)
    claims = sum(r["claims_total"] for r in rows)
    center_claims = sum(r["center_claims"] for r in rows)
    center_caps = sum(r["center_caps"] for r in rows)
    vacates = sum(r["vacates"] for r in rows)
    holders = sum(r["holders"] for r in rows)
    adj = sum(r["adjacent_setups"] for r in rows)
    mean_turns = sum(r["turns"] or 0 for r in rows) / n if n else 0
    mean_first_turn = (
        sum(r["first_turn"] or 0 for r in took) / len(took) if took else 0
    )

    print(f"file {path}")
    print(f"games {n}  mean_turns {mean_turns:.2f}")
    print(f"winners {dict(winners)}  reasons {dict(reasons)}")
    print(
        f"first_center_taker {len(took)}/{n}  mean_first_turn {mean_first_turn:.2f}"
    )
    print(
        f"still_on_center {len(still)}/{len(took)} "
        f"({(len(still)/len(took)*100 if took else 0):.1f}%)"
    )
    print(
        f"never_left {len(never_left)}/{len(took)} "
        f"({(len(never_left)/len(took)*100 if took else 0):.1f}%)"
    )
    print(
        f"first_taker_side_won {len(first_wins)}/{len(took)} "
        f"({(len(first_wins)/len(took)*100 if took else 0):.1f}%)"
    )
    print(
        f"still_and_that_side_won {len(still_and_win)}/{len(still)} "
        f"({(len(still_and_win)/len(still)*100 if still else 0):.1f}%)"
    )
    print(
        f"displaced {len(displaced)}  displaced_but_first_side_won {len(displaced_first_still_wins)}"
    )
    print(
        f"center_occupants_over_game {holders}  vacates {vacates}  "
        f"center_opp_caps {center_caps}  center_claims {center_claims}  all_claims {claims}  "
        f"adj_setups_before_first_take {adj}"
    )

    def show(label: str, subset: list[dict], k: int = 6) -> None:
        print(f"\n# examples {label}")
        for r in subset[:k]:
            print(
                f"  game={r['game']} {r['pairing']} {r['matchup']} turns={r['turns']} "
                f"end={r['reason']} W={r['winner']} first=P{r['first_owner']}@t{r['first_turn']} "
                f"{r['first_kind']} still={r['still_first']} holders={r['holders']} "
                f"vacates={r['vacates']} c_caps={r['center_caps']} claims={r['claims_total']}"
            )

    show("still_on_center_and_win", still_and_win)
    show("still_on_center_but_lose", [r for r in still if not r["first_side_won"]])
    show("displaced_and_other_side_won", [r for r in displaced if not r["first_side_won"]])
    show("center_claims", [r for r in rows if r["center_claims"] > 0])
    show("many_holders", sorted(rows, key=lambda r: -(r["holders"] or 0)))


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: survey_center.py <games.jsonl.gz> [...]", file=sys.stderr)
        sys.exit(2)
    for arg in sys.argv[1:]:
        summarize(Path(arg))
        print()


if __name__ == "__main__":
    main()
