"""Time to the first hover in the editor: open a file, then hover inside one of its functions.

Usage: python bench/first_hover.py ROWAN NAME [--modules K] [--out DIR]

For K modules of the benchmark program (spread over its files, as serve_hovers.py picks them)
it starts a `rowan serve` (as the extension does), sends `{"cmd":"analyse"}` for the file, and
once that is answered a hover inside a function of the file (the middle one): the time to the
first hover is from the analyse request to the hover's answer. When the analysis is on demand
(`on_demand`), the hover runs the function; otherwise the analyse document answers it. Also
the server's peak memory (`/usr/bin/time -l`).

Prints per module the analyse time, whether it went on demand, the hover time and the total;
then p50 / p95 / max of the totals. Writes DIR/NAME-first.json.
"""

import argparse
import json
import os
import re
import subprocess
import time
from pathlib import Path

from serve_hovers import functions, pct

HERE = Path(__file__).resolve().parent
GUARD = HERE.parent / ".guard" / "run"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("rowan")
    ap.add_argument("name")
    ap.add_argument("--modules", type=int, default=12)
    ap.add_argument("--out", default="bench-results")
    a = ap.parse_args()
    root = HERE / "work" / a.name
    files = sorted(p for p in root.rglob("*.py") if not p.name.startswith("_") and p.name != "main.py")
    files = [f for f in files if len(functions(f, 1)) > 0]
    if len(files) > a.modules:
        step = len(files) / a.modules
        files = [files[int(i * step)] for i in range(a.modules)]
    env = dict(os.environ)
    env.pop("ROWAN_DEBUG", None)
    rows = []
    for f in files:
        fns = functions(f, 3)
        qn, line = fns[len(fns) // 2]
        p = subprocess.Popen([str(GUARD), "-t", "0", "/usr/bin/time", "-l", a.rowan, "serve"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
        t0 = time.monotonic()
        p.stdin.write(json.dumps({"id": 1, "cmd": "analyse", "path": str(f)}) + "\n")
        p.stdin.flush()
        r = json.loads(p.stdout.readline())
        analyse_ms = (time.monotonic() - t0) * 1000
        demand = bool(r.get("ok") and r["result"].get("on_demand"))
        hover_ms = 0.0
        if demand:
            t1 = time.monotonic()
            p.stdin.write(json.dumps({"id": 2, "cmd": "hover", "path": str(f), "line": line}) + "\n")
            p.stdin.flush()
            h = json.loads(p.stdout.readline())
            hover_ms = (time.monotonic() - t1) * 1000
            if not h.get("ok"):
                hover_ms = -1
        total = (time.monotonic() - t0) * 1000
        p.stdin.write(json.dumps({"id": -1, "cmd": "shutdown"}) + "\n")
        p.stdin.flush()
        _, err = p.communicate()
        m = re.search(r"(\d+)\s+maximum resident set size", err)
        mb = int(m.group(1)) / 1e6 if m else None
        row = {"file": str(f.relative_to(root)), "function": qn, "line": line, "ok": r.get("ok"), "analyse_ms": round(analyse_ms), "on_demand": demand, "hover_ms": round(hover_ms), "total_ms": round(total), "peak_mb": mb and round(mb)}
        print(f"{row['file']}: analyse {row['analyse_ms']} ms{' (on demand)' if demand else ''}, hover {row['hover_ms']} ms, first hover at {row['total_ms']} ms, {row['peak_mb']} MB", flush=True)
        rows.append(row)
    tot = [r["total_ms"] for r in rows]
    mem = [r["peak_mb"] or 0 for r in rows]
    summary = {"name": a.name, "modules": len(rows), "on_demand": sum(r["on_demand"] for r in rows), "first_hover_ms": {"p50": pct(tot, .5), "p95": pct(tot, .95), "max": max(tot, default=0)}, "peak_mb": max(mem, default=0)}
    print(json.dumps(summary))
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / f"{a.name}-first.json").write_text(json.dumps({"summary": summary, "modules": rows}, indent=1))


if __name__ == "__main__":
    main()
