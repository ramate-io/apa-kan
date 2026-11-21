#!/usr/bin/env python3
"""Hillclimb existing SearchConfig *eval* weights from H2H win rate.

Only the leaf knobs already on SearchConfig are searched:

  eval_margin_scale, eval_center_occ, eval_inner_ring, eval_center_pressure

No pick bonuses, extra qsearch, move-order tactics, priors, or subsearch.

Each candidate plays the current champion in *both seats*. Score is the
candidate's combined W-D-L. A candidate replaces the champion only if it
is strictly ahead on wins-minus-losses by `--promote-net` (default 4) on
the climb sample.

Repro (the committed climb):

  python3 apa-kan/game/benchmark_results/exactplay/tune_eval.py climb \\
    --start exactplay --seed 7 --games 10 \\
    --budget nodes:12000,iters:250 --jobs 4 --rounds 3 --promote-net 4 \\
    --work apa-kan/game/benchmark_results/exactplay/tune

  python3 apa-kan/game/benchmark_results/exactplay/tune_eval.py confirm \\
    --confirm-games 50 --confirm-budget nodes:33000,iters:650 --confirm-seed 1 \\
    --work apa-kan/game/benchmark_results/exactplay/tune

Climb step `s` uses bench `--seed = base + 17*(s+1)` (same-deck deal).
Isolation (`run_isolation.sh`) and vs-counterplay (`run_h2h.sh`) use `--seed 1`.
When the climb does not promote, those 100-game cells *are* the confirmation
(same champion, same budget); `confirm` re-runs them under `tune/`.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
BIN_DEFAULT = REPO / "target/release/apa-kan-bench"

# Leaf-eval grid. Neighbors of the current champion are tried, not the
# cartesian product. Values are the live enhanced/exactplay/counterplay
# settings plus one step each side.
GRID = {
    "eval_margin_scale": [100, 150, 200, 250, 300],
    "eval_center_occ": [70, 120, 180, 240],
    "eval_inner_ring": [40, 55, 70, 90],
    "eval_center_pressure": [0, 50, 100],
}

ENHANCED = {
    "eval_margin_scale": 100,
    "eval_center_occ": 180,
    "eval_inner_ring": 55,
    "eval_center_pressure": 0,
}
EXACTPLAY = {**ENHANCED, "eval_margin_scale": 200}
COUNTERPLAY = {
    "eval_margin_scale": 100,
    "eval_center_occ": 70,
    "eval_inner_ring": 70,
    "eval_center_pressure": 100,
}

TOML_KEYS = (
    "eval_margin_scale",
    "eval_center_occ",
    "eval_inner_ring",
    "eval_center_pressure",
)

SEAT_TOTALS = re.compile(
    r"# seat_totals\s+P1\s+(\d+)-(\d+)-(\d+)\s+P2\s+(\d+)-(\d+)-(\d+)\s+games=(\d+)"
)


@dataclass(frozen=True)
class Weights:
    eval_margin_scale: int
    eval_center_occ: int
    eval_inner_ring: int
    eval_center_pressure: int

    def as_dict(self) -> dict[str, int]:
        return asdict(self)

    def label(self) -> str:
        d = self.as_dict()
        return ",".join(f"{k}={d[k]}" for k in TOML_KEYS)

    def slug(self) -> str:
        d = self.as_dict()
        return "m{eval_margin_scale}_c{eval_center_occ}_i{eval_inner_ring}_p{eval_center_pressure}".format(
            **d
        )


def weights_from(d: dict[str, int]) -> Weights:
    return Weights(**{k: int(d[k]) for k in TOML_KEYS})


def write_toml(path: Path, w: Weights, name: str, description: str) -> None:
    lines = [
        f'name = "{name}"',
        f'description = "{description}"',
    ]
    for k in TOML_KEYS:
        lines.append(f"{k} = {getattr(w, k)}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def parse_seat_totals(text: str) -> tuple[int, int, int, int]:
    matches = SEAT_TOTALS.findall(text)
    if not matches:
        raise RuntimeError("no # seat_totals line in bench output")
    w, d, l, _p2w, _p2d, _p2l, games = (int(x) for x in matches[-1])
    return w, d, l, games


def bench_cmd(
    bin_path: Path,
    *,
    games: int,
    seed: int,
    budget: str,
    jobs: int,
    p1_toml: Path,
    p2_toml: Path,
    out_txt: Path,
    record: Path,
    log: Path,
    label: str,
) -> list[str]:
    return [
        str(bin_path),
        "--budget",
        budget,
        "--games",
        str(games),
        "--jobs",
        str(jobs),
        "--no-depth-probe",
        "--zags",
        "on",
        "--zag-mismatch",
        "sum",
        "--endgame-rule",
        "control-cascade",
        "--cascade-turn-gain",
        "on",
        "--capture-mode",
        "local-claim",
        "--no-same-piece-twice",
        "off",
        "--pick",
        "search",
        "--pairings",
        "minimax:mcts,mcts:minimax",
        "--same-deck",
        "on",
        "--seed",
        str(seed),
        "--p1-bot-config",
        str(p1_toml),
        "--p2-bot-config",
        str(p2_toml),
        "--out",
        str(out_txt),
        "--record",
        str(record),
        "--games-log",
        str(log),
        "--label",
        label,
    ]


def run_cell(
    bin_path: Path,
    work: Path,
    *,
    name: str,
    games: int,
    seed: int,
    budget: str,
    jobs: int,
    p1: Weights,
    p2: Weights,
) -> dict:
    work.mkdir(parents=True, exist_ok=True)
    p1_toml = work / f"{name}_p1.toml"
    p2_toml = work / f"{name}_p2.toml"
    write_toml(p1_toml, p1, "p1", "hillclimb p1")
    write_toml(p2_toml, p2, "p2", "hillclimb p2")
    out_txt = work / f"{name}.txt"
    record = work / f"{name}_record.txt"
    log = work / f"{name}.jsonl.gz"
    cmd = bench_cmd(
        bin_path,
        games=games,
        seed=seed,
        budget=budget,
        jobs=jobs,
        p1_toml=p1_toml,
        p2_toml=p2_toml,
        out_txt=out_txt,
        record=record,
        log=log,
        label=f"tune {name}",
    )
    print("+", " ".join(cmd), flush=True)
    proc = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True)
    combined = (proc.stdout or "") + "\n" + (proc.stderr or "")
    if proc.returncode != 0:
        raise RuntimeError(f"bench failed ({proc.returncode}):\n{combined[-4000:]}")
    text = out_txt.read_text(encoding="utf-8") if out_txt.exists() else combined
    w, d, l, n = parse_seat_totals(text)
    return {
        "name": name,
        "p1": p1.as_dict(),
        "p2": p2.as_dict(),
        "p1_wdl": [w, d, l],
        "games": n,
        "seed": seed,
        "txt": str(out_txt),
        "log": str(log),
    }


def h2h_both_seats(
    bin_path: Path,
    work: Path,
    *,
    tag: str,
    cand: Weights,
    opp: Weights,
    games: int,
    seed: int,
    budget: str,
    jobs: int,
) -> dict:
    """Candidate vs opponent in both seats. Score is from the candidate's view."""
    a = run_cell(
        bin_path,
        work,
        name=f"{tag}_cand_p1",
        games=games,
        seed=seed,
        budget=budget,
        jobs=jobs,
        p1=cand,
        p2=opp,
    )
    b = run_cell(
        bin_path,
        work,
        name=f"{tag}_cand_p2",
        games=games,
        seed=seed + 1,
        budget=budget,
        jobs=jobs,
        p1=opp,
        p2=cand,
    )
    # cand wins = P1 wins in A + P2 wins in B = A.w + B.l
    aw, ad, al = a["p1_wdl"]
    bw, bd, bl = b["p1_wdl"]
    w = aw + bl
    d = ad + bd
    l = al + bw
    n = a["games"] + b["games"]
    return {
        "tag": tag,
        "cand": cand.as_dict(),
        "opp": opp.as_dict(),
        "wdl": [w, d, l],
        "net": w - l,
        "games": n,
        "as_p1": a,
        "as_p2": b,
        "seed": seed,
    }


def neighbors(champ: Weights) -> list[tuple[str, Weights]]:
    out: list[tuple[str, Weights]] = []
    d = champ.as_dict()
    for key, grid in GRID.items():
        cur = d[key]
        if cur not in grid:
            grid = sorted(set(grid) | {cur})
        idx = grid.index(cur)
        for j in (idx - 1, idx + 1):
            if 0 <= j < len(grid) and grid[j] != cur:
                nd = dict(d)
                nd[key] = grid[j]
                out.append((f"{key}:{cur}->{grid[j]}", weights_from(nd)))
    return out


def append_jsonl(path: Path, row: dict) -> None:
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(row, sort_keys=True) + "\n")


def climb(args: argparse.Namespace) -> Weights:
    bin_path = Path(args.bin)
    work = Path(args.work)
    work.mkdir(parents=True, exist_ok=True)
    traj = work / "trajectory.jsonl"
    champ = weights_from(EXACTPLAY if args.start == "exactplay" else ENHANCED)
    if args.start == "enhanced":
        champ = weights_from(ENHANCED)
    elif args.start == "counterplay":
        champ = weights_from(COUNTERPLAY)
    print(f"# start champion {champ.label()}", flush=True)
    append_jsonl(
        traj,
        {"event": "start", "champion": champ.as_dict(), "seed": args.seed, "games": args.games},
    )
    step = 0
    for round_i in range(args.rounds):
        improved = False
        for move, cand in neighbors(champ):
            if cand == champ:
                continue
            tag = f"r{round_i}_s{step}_{cand.slug()}"
            seed = args.seed + 17 * (step + 1)
            print(f"# try {move} seed={seed} vs {champ.label()}", flush=True)
            result = h2h_both_seats(
                bin_path,
                work,
                tag=tag,
                cand=cand,
                opp=champ,
                games=args.games,
                seed=seed,
                budget=args.budget,
                jobs=args.jobs,
            )
            result["event"] = "try"
            result["round"] = round_i
            result["step"] = step
            result["move"] = move
            result["champion"] = champ.as_dict()
            append_jsonl(traj, result)
            w, d, l = result["wdl"]
            print(
                f"#   cand {w}-{d}-{l} net={result['net']}  promote>={args.promote_net}",
                flush=True,
            )
            step += 1
            if result["net"] >= args.promote_net:
                champ = cand
                improved = True
                append_jsonl(
                    traj,
                    {"event": "promote", "champion": champ.as_dict(), "move": move, "wdl": result["wdl"]},
                )
                print(f"# promote {champ.label()}", flush=True)
                break
        if not improved:
            print(f"# round {round_i} local peak {champ.label()}", flush=True)
            break
    write_toml(
        work / "champion.toml",
        champ,
        "exactplay",
        "Hillclimbed leaf eval (see trajectory.jsonl)",
    )
    append_jsonl(traj, {"event": "done", "champion": champ.as_dict(), "steps": step})
    print(f"# done champion {champ.label()}", flush=True)
    return champ


def confirm(args: argparse.Namespace, champ: Weights | None = None) -> None:
    bin_path = Path(args.bin)
    work = Path(args.work)
    work.mkdir(parents=True, exist_ok=True)
    if champ is None:
        toml_path = work / "champion.toml"
        if not toml_path.exists():
            raise SystemExit("confirm needs climb first (missing champion.toml)")
        body = toml_path.read_text(encoding="utf-8")
        vals = {}
        for k in TOML_KEYS:
            m = re.search(rf"^{k}\s*=\s*(-?\d+)", body, re.M)
            if not m:
                raise SystemExit(f"champion.toml missing {k}")
            vals[k] = int(m.group(1))
        champ = weights_from(vals)
    traj = work / "trajectory.jsonl"
    for opp_name, opp in (("enhanced", weights_from(ENHANCED)), ("counterplay", weights_from(COUNTERPLAY))):
        tag = f"confirm_{champ.slug()}_vs_{opp_name}"
        result = h2h_both_seats(
            bin_path,
            work,
            tag=tag,
            cand=champ,
            opp=opp,
            games=args.games,
            seed=args.seed,
            budget=args.budget,
            jobs=args.jobs,
        )
        result["event"] = "confirm"
        result["opp_name"] = opp_name
        append_jsonl(traj, result)
        w, d, l = result["wdl"]
        print(f"# confirm vs {opp_name}: {w}-{d}-{l} net={result['net']}", flush=True)


def build_bin() -> None:
    cmd = [
        "cargo",
        "build",
        "--release",
        "--no-default-features",
        "-p",
        "apa-kan",
        "--bin",
        "apa-kan-bench",
    ]
    try:
        subprocess.check_call(cmd + ["--offline"], cwd=REPO)
    except subprocess.CalledProcessError:
        subprocess.check_call(cmd, cwd=REPO)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("cmd", choices=("climb", "confirm", "all"), help="climb, confirm, or both")
    p.add_argument("--bin", default=str(BIN_DEFAULT))
    p.add_argument("--work", default=str(OUT / "tune"))
    p.add_argument("--seed", type=int, default=7, help="climb seed base (confirm default overridden below)")
    p.add_argument("--games", type=int, default=8, help="games per pairing per seat (climb)")
    p.add_argument("--budget", default="nodes:12000,iters:250", help="climb work budget")
    p.add_argument("--jobs", type=int, default=4)
    p.add_argument("--rounds", type=int, default=3, help="coordinate-descent passes")
    p.add_argument("--promote-net", type=int, default=4, help="min (W-L) to replace champion")
    p.add_argument("--start", choices=("exactplay", "enhanced", "counterplay"), default="exactplay")
    p.add_argument("--confirm-games", type=int, default=50, help="games per pairing per seat for confirm")
    p.add_argument("--confirm-budget", default="nodes:33000,iters:650")
    p.add_argument("--confirm-seed", type=int, default=1)
    args = p.parse_args()
    if not Path(args.bin).exists():
        build_bin()
    if args.cmd in ("climb", "all"):
        champ = climb(args)
    else:
        champ = None
    if args.cmd in ("confirm", "all"):
        cargs = argparse.Namespace(**vars(args))
        cargs.games = args.confirm_games
        cargs.budget = args.confirm_budget
        cargs.seed = args.confirm_seed
        confirm(cargs, champ)
    return 0


if __name__ == "__main__":
    sys.exit(main())
