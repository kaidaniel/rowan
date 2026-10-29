"""The protocol enumerators (tests/*_protocol.py), as CI runs them and in full.

  ci    every enumerator step of .github/workflows/ci.yml at its CI bound, read from ci.yml
        itself (so nothing there is left out here): its arguments, its `--with` libraries,
        and the PyTorch CPU index for torch
  full  the steps whose CI bound is a sample (`--rowan N`) on every program instead (`--all`;
        try_protocol.py, which has no `--all`, with a sample larger than its grammar), each
        split into shards (`--shard K/N`) of about UNIT program runs. `plan --jobs J` deals the
        shards to J jobs, largest first (bench.yml's matrix); `--job K/J` runs the K-th job's.
        The steps CI already runs whole (`--all`) are not run again.

    uv run --no-project python bench/protocols.py ci [--only NAME,…] [-j P] [--rowan BIN] [--json OUT]
    uv run --no-project python bench/protocols.py full [--job K/J] [--only NAME,…] [-j P] [--rowan BIN] [--json OUT]
    uv run --no-project python bench/protocols.py plan --jobs J

Each step runs as its own process (`uv run --no-project [--with …] python tests/X_protocol.py
ARGS`), P at once (each enumerator runs rowan through .guard/run itself where it exists; do not
wrap this script in the guard). Its report lines (`rowan[ (ROWAN_LIMITS=…)]: N programs, P points
checked, M with a run rowan's claims miss …; rowan failed on C batches`) are summed per step:
programs, points, missed, crashed batches, seconds, the exit code. Exit 1 when any step missed a
point, crashed or failed.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CI = ROOT / ".github" / "workflows" / "ci.yml"
TORCH_INDEX = "https://download.pytorch.org/whl/cpu"
STEP = re.compile(r"^\s*run: uv run --no-project((?: --with [\w-]+)*) python (tests/(\w+)_protocol\.py)((?: [-\w]+)*)\s*$")
REPORT = re.compile(r"^rowan(?: \(ROWAN_LIMITS=[^)]*\))?: (\d+) programs, (\d+) points checked, (\d+) with a run rowan's claims miss[^;\n]*(?:;[^;\n]*; rowan failed on (\d+) batches)?", re.M)
# program runs (programs x limit sets) of each step run whole, counted with `--rowan 0` at
# v0.1.103: what `plan` deals by (a step not listed counts DEFAULT_RUNS)
RUNS = {"try": 24_304, "gen": 57_888, "with": 6_704, "iter": 3_562, "call": 213_192, "rec": 7_776, "method": 50_472,
        "closure": 4_680, "narrow": 14_300, "anycall": 308_448, "deferred": 53_100, "narrow-pairs": 89_760,
        "dunder-pairs": 23_100, "class-pairs": 51_084, "container-pairs": 7_569, "call-pairs": 15_360}
DEFAULT_RUNS = 10_000
UNIT = 20_000  # program runs per shard: about 2 minutes on a CI runner


def ci_steps() -> list[dict]:
    """ci.yml's enumerator steps: {name, script, args, deps}; `name` is the enumerator, `-pairs`
    for its pair mode."""
    out = []
    for line in CI.read_text().splitlines():
        m = STEP.match(line)
        if not m:
            continue
        deps = m.group(1).split()[1::2]
        args = m.group(4).split()
        name = m.group(3) + ("-pairs" if "--pairs" in args else "")
        out.append({"name": name, "script": m.group(2), "args": args, "deps": deps})
    return out


def full_steps() -> list[dict]:
    """The steps CI bounds by a sample, on every program, in shards."""
    out = []
    for s in ci_steps():
        if "--rowan" not in s["args"]:
            continue
        i = s["args"].index("--rowan")
        rest = s["args"][:i] + s["args"][i + 2:]
        if s["name"] == "try":
            # (no --all, no shards: a sample larger than its grammar is all of it)
            out.append({**s, "args": rest + ["--rowan", "1000000"], "shard": None})
            continue
        n = max(1, math.ceil(RUNS.get(s["name"], DEFAULT_RUNS) / UNIT))
        out += [{**s, "args": rest + ["--all"] + (["--shard", f"{k}/{n}"] if n > 1 else []), "shard": f"{k}/{n}" if n > 1 else None} for k in range(n)]
    return out


def weight(s: dict) -> float:
    n = int(s["shard"].split("/")[1]) if s["shard"] else 1
    return RUNS.get(s["name"], DEFAULT_RUNS) / n


def plan(jobs: int) -> list[list[dict]]:
    """The full steps dealt to `jobs` jobs, largest first, each to the least loaded."""
    out: list[list[dict]] = [[] for _ in range(jobs)]
    load = [0.0] * jobs
    for s in sorted(full_steps(), key=lambda s: -weight(s)):
        k = load.index(min(load))
        out[k].append(s)
        load[k] += weight(s)
    return out


def label(s: dict) -> str:
    return " ".join([s["script"], *s["args"]])


def run_step(s: dict, rowan: str | None, logdir: Path | None) -> dict:
    deps = [x for d in s["deps"] for x in ("--with", d)]
    env = dict(os.environ)
    if rowan:
        env["ROWAN_BIN"] = str(Path(rowan).resolve())
    if "torch" in s["deps"]:
        env.setdefault("UV_EXTRA_INDEX_URL", TORCH_INDEX)
    t0 = time.monotonic()
    r = subprocess.run(["uv", "run", "--no-project", *deps, "python", s["script"], *s["args"]], cwd=ROOT, env=env, capture_output=True, text=True,
                       stdin=subprocess.DEVNULL)
    secs = round(time.monotonic() - t0, 1)
    found = REPORT.findall(r.stdout)
    res = {"name": s["name"], "shard": s.get("shard"), "cmd": label(s), "rc": r.returncode, "seconds": secs,
           "programs": sum(int(x[0]) for x in found), "points": sum(int(x[1]) for x in found), "missed": sum(int(x[2]) for x in found),
           "crashed": sum(int(x[3] or 0) for x in found), "reports": len(found)}
    if logdir:
        logdir.mkdir(parents=True, exist_ok=True)
        (logdir / (s["name"] + (f"-{s['shard'].replace('/', 'of')}" if s.get("shard") else "") + ".log")).write_text(r.stdout + r.stderr)
    if r.returncode or not found:
        res["tail"] = (r.stdout + r.stderr)[-1500:]
    return res


def run_all(steps: list[dict], jobs: int, rowan: str | None, logdir: Path | None) -> list[dict]:
    def one(s):
        r = run_step(s, rowan, logdir)
        bad = r["rc"] or r["missed"] or r["crashed"] or not r["reports"]
        print(f"  {r['cmd']:<70} {r['programs']:>7} programs {r['points']:>8} points {r['missed']:>4} missed {r['crashed']} crashed "
              f"{r['seconds']:>7.1f} s{'  FAILED' if bad else ''}", file=sys.stderr, flush=True)
        return r

    with ThreadPoolExecutor(jobs) as ex:
        return list(ex.map(one, steps))


def summary(results: list[dict]) -> dict:
    """Per enumerator (shards summed): programs, points, missed, crashed, seconds, failed."""
    out: dict = {}
    for r in results:
        t = out.setdefault(r["name"], {"programs": 0, "points": 0, "missed": 0, "crashed": 0, "seconds": 0.0, "failed": 0, "steps": 0})
        for k in ("programs", "points", "missed", "crashed", "seconds"):
            t[k] += r[k]
        t["steps"] += 1
        t["failed"] += bool(r["rc"] or r["missed"] or r["crashed"] or not r["reports"])
        t["seconds"] = round(t["seconds"], 1)
    return out


def finish(a, kind: str, results: list[dict], t0: float) -> None:
    s = summary(results)
    doc = {"kind": kind, "job": getattr(a, "job", None), "wall_s": round(time.monotonic() - t0, 1), "steps": results, "by_name": s}
    if a.json:
        Path(a.json).parent.mkdir(parents=True, exist_ok=True)
        Path(a.json).write_text(json.dumps(doc, indent=1))
    bad = {n: t for n, t in s.items() if t["failed"]}
    print(f"protocols {kind}: {len(results)} steps, {sum(t['points'] for t in s.values()):,} points, "
          f"{sum(t['missed'] for t in s.values())} missed, {len(bad)} enumerators failed; {doc['wall_s']} s")
    for n, t in bad.items():
        print(f"  FAILED {n}: {t['missed']} missed, {t['crashed']} crashed batches")
    sys.exit(1 if bad else 0)


def select(steps: list[dict], only: str | None) -> list[dict]:
    return [s for s in steps if not only or s["name"] in only.split(",")]


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("ci", "full"):
        q = sub.add_parser(name)
        q.add_argument("--only")
        q.add_argument("-j", type=int, default=3)
        q.add_argument("--rowan")
        q.add_argument("--json")
        q.add_argument("--logs")
        if name == "full":
            q.add_argument("--job", help="K/J: the K-th of J jobs of `plan`")
    q = sub.add_parser("plan")
    q.add_argument("--jobs", type=int, required=True)
    a = ap.parse_args()
    if a.cmd == "plan":
        p = plan(a.jobs)
        print(json.dumps([{"job": f"{k}/{a.jobs}", "steps": len(js), "runs": round(sum(weight(s) for s in js))} for k, js in enumerate(p)]))
        return
    t0 = time.monotonic()
    logs = Path(a.logs) if a.logs else None
    if a.cmd == "ci":
        finish(a, "ci", run_all(select(ci_steps(), a.only), a.j, a.rowan, logs), t0)
    steps = full_steps()
    if a.job:
        k, j = map(int, a.job.split("/"))
        steps = plan(j)[k]
    finish(a, "full", run_all(select(steps, a.only), a.j, a.rowan, logs), t0)


if __name__ == "__main__":
    main()
