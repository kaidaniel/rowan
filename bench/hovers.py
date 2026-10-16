"""Time to information on demand: every function of a benchmark program run alone, timed.

Usage: python bench/hovers.py ROWAN NAME [-j N] [--limits L] [--out DIR]

Sets up bench/work/NAME/_all_modules.py (hover_setup.py: an import of every module) in N
guarded `rowan hovers` processes (each does the setup once, then every Nth function), and
prints the setup time, p50 / p95 / p99 / max of the per-function times, a histogram, and the
slowest 20 functions. Writes DIR/NAME-hovers.json.
"""

import argparse
import json
import os
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
GUARD = HERE.parent / ".guard" / "run"


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(p * (len(xs) - 1))))] if xs else 0.0


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("rowan")
    ap.add_argument("name")
    ap.add_argument("-j", type=int, default=3)
    ap.add_argument("--limits", default="")
    ap.add_argument("--out", default="bench-results")
    a = ap.parse_args()
    setup_file = subprocess.run(["uv", "run", "--no-project", "python", str(HERE / "hover_setup.py"), a.name], capture_output=True, text=True, check=True).stdout.strip()
    env = dict(os.environ)
    env.pop("ROWAN_DEBUG", None)
    if a.limits:
        env["ROWAN_LIMITS"] = a.limits
    procs = [subprocess.Popen([str(GUARD), "-t", "0", a.rowan, "hovers", setup_file, "--part", f"{i}/{a.j}"], stdout=subprocess.PIPE, text=True, env=env) for i in range(a.j)]
    setups, rows, rebuilds = [], [], []
    for p in procs:
        out, _ = p.communicate()
        lines = [json.loads(l) for l in out.splitlines() if l.strip()]
        if lines:
            setups.append(lines[0])
            rows += [l for l in lines[1:] if "ms" in l]
            rebuilds += [l["rebuild_ms"] for l in lines[1:] if "rebuild_ms" in l]
    ms = [r["ms"] for r in rows]
    s0 = setups[0] if setups else {}
    summary = {
        "name": a.name, "functions": s0.get("functions"), "timed": len(rows), "setup_ms": s0.get("setup_ms"), "parse_ms": s0.get("parse_ms"),
        "modules": s0.get("modules"), "p50": pct(ms, .5), "p95": pct(ms, .95), "p99": pct(ms, .99), "max": max(ms, default=0),
        "over_1s": sum(1 for x in ms if x > 1000), "cut": sum(1 for r in rows if r["cut"]), "total_s": round(sum(ms) / 1000, 1), "rebuilds": len(rebuilds), "rebuild_ms": max(rebuilds, default=0),
    }
    bins = [1, 10, 100, 300, 1000, 3000, 10000, float("inf")]
    hist, lo = {}, 0
    for b in bins:
        hist[f"<{b} ms" if b != float("inf") else f">={lo} ms"] = sum(1 for x in ms if lo <= x < b)
        lo = b
    summary["histogram"] = hist
    slow = sorted(rows, key=lambda r: -r["ms"])[:20]
    print(json.dumps(summary))
    for r in slow:
        print(f"  {r['ms']:9.1f} ms {r['statements']:8d} stmts {r['havoc']:9d} havoc {'cut ' if r['cut'] else ''}{r['file']}:{r['line']} {r['function']}")
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / f"{a.name}-hovers.json").write_text(json.dumps({"summary": summary, "setups": setups, "functions": rows}, indent=1))


if __name__ == "__main__":
    main()
