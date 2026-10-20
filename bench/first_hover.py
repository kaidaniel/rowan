"""Time to the first hover in the editor: open a file, then hover inside one of its functions.

Usage: python bench/first_hover.py ROWAN NAME [--modules K] [--out DIR] [--follow PKGS]
       python bench/first_hover.py ROWAN sitepackages [--scenarios s1,…] [--follow PKGS]

For K modules of the benchmark program (spread over its files, as serve_hovers.py picks them)
it starts a `rowan serve` (as the extension does), sends `{"cmd":"analyse"}` for the file, and
once that is answered a hover inside a function of the file (the middle one): the time to the
first hover is from the analyse request to the hover's answer. When the analysis is on demand
(`on_demand`), the hover runs the function; otherwise the analyse document answers it. Also
the server's peak memory (`/usr/bin/time`: `-l` on macOS, `-v` on Linux).

Prints per module the analyse time, whether it went on demand, the hover time and the total;
then p50 / p95 / max of the totals. Writes DIR/NAME-first.json.

With `--follow PKGS` the server follows installed packages (`--follow`, and for the
`sitepackages` name each scenario's main file of bench/sitepackages.py with its venv's
site-packages, `--follow '*'` unless given): it answers first without following, then sends the
followed document as an event; the time to that event (`followed_ms`, from the analyse
request; -1 when none came within `--followed-timeout`) is printed too.
"""

import argparse
import json
import os
import queue
import re
import subprocess
import sys
import threading
import time
from pathlib import Path

from serve_hovers import functions, pct, reply

HERE = Path(__file__).resolve().parent
GUARD = HERE.parent / ".guard" / "run"


# peak memory: BSD time's `-l` (bytes), GNU time's `-v` (kbytes); left out where there is none
TIMER = (["/usr/bin/time", "-l"] if sys.platform == "darwin" else ["/usr/bin/time", "-v"]) if os.path.exists("/usr/bin/time") else []


def guarded(t: int) -> list[str]:
    """The guard's prefix on the owner's Mac (.guard/run, not in the repository); none elsewhere (CI)."""
    return [str(GUARD), "-t", str(t)] if GUARD.exists() else []


def lines_of(p: subprocess.Popen) -> "queue.Queue[dict | None]":
    """The server's output lines as they come (None when it exits), read on a thread."""
    q: "queue.Queue[dict | None]" = queue.Queue()

    def read() -> None:
        for line in p.stdout:
            try:
                q.put(json.loads(line))
            except json.JSONDecodeError:
                pass
        q.put(None)

    threading.Thread(target=read, daemon=True).start()
    return q


def answer(q: "queue.Queue[dict | None]", rid: int, seen: list[dict]) -> dict:
    """The answer to request `rid` (lines before it, events, go to `seen`, timed)."""
    while True:
        r = q.get()
        if r is None:
            return {"ok": False, "error": "the server exited"}
        if r.get("id") == rid:
            return r
        seen.append(dict(r, at=time.monotonic()))


def targets(a) -> list[tuple[Path, Path, list[str]]]:
    """(file, root it is shown relative to, serve's extra arguments) for each file measured."""
    follow = ["--follow", a.follow] if a.follow else []
    if a.name == "sitepackages":
        import sitepackages as sp
        out = []
        for name, spec in sp.SCENARIOS.items():
            if a.scenarios and not any(name == s or name.startswith(s + "_") for s in a.scenarios.split(",")):
                continue
            site = sp.site_packages(sp.venv_for(name, spec))
            out.append((sp.PROGS / f"{name}.py", sp.PROGS, ["--site-packages", str(site)] + (follow or ["--follow", "*"]) * (a.follow != "")))
        return out
    root = HERE / "work" / a.name
    files = sorted(p for p in root.rglob("*.py") if not p.name.startswith("_") and p.name != "main.py")
    files = [f for f in files if len(functions(f, 1)) > 0]
    if len(files) > a.modules:
        step = len(files) / a.modules
        files = [files[int(i * step)] for i in range(a.modules)]
    return [(f, root, follow) for f in files]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("rowan")
    ap.add_argument("name")
    ap.add_argument("--modules", type=int, default=12)
    ap.add_argument("--out", default="bench-results")
    ap.add_argument("--follow", default=None, help="serve's --follow (sitepackages: '*' unless given; '' for none)")
    ap.add_argument("--scenarios", default=None, help="sitepackages: which scenarios (s1,s2,…)")
    ap.add_argument("--followed-timeout", type=float, default=60)
    a = ap.parse_args()
    env = dict(os.environ)
    env.pop("ROWAN_DEBUG", None)
    rows = []
    for f, root, extra in targets(a):
        fns = functions(f, 3) or functions(f, 1)
        qn, line = fns[len(fns) // 2] if fns else ("", 1)
        p = subprocess.Popen(guarded(0) + TIMER + [a.rowan, "serve", *extra], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
        q, seen = lines_of(p), []
        t0 = time.monotonic()
        p.stdin.write(json.dumps({"id": 1, "cmd": "analyse", "path": str(f)}) + "\n")
        p.stdin.flush()
        r = answer(q, 1, seen)
        analyse_ms = (time.monotonic() - t0) * 1000
        demand = bool(r.get("ok") and r["result"].get("on_demand"))
        hover_ms = 0.0
        if demand and fns:
            t1 = time.monotonic()
            p.stdin.write(json.dumps({"id": 2, "cmd": "hover", "path": str(f), "line": line}) + "\n")
            p.stdin.flush()
            h = answer(q, 2, seen)
            hover_ms = (time.monotonic() - t1) * 1000
            if not h.get("ok"):
                hover_ms = -1
        total = (time.monotonic() - t0) * 1000
        # (following: the followed document, as an event, unless the answer was it already)
        followed_ms, followed_facts = None, None
        if "--follow" in extra and r.get("ok"):
            ev = next((e for e in seen if e.get("followed")), None)
            deadline = time.monotonic() + a.followed_timeout
            while ev is None and r.get("following") and time.monotonic() < deadline:
                try:
                    e = q.get(timeout=max(0.01, deadline - time.monotonic()))
                except queue.Empty:
                    break
                if e is None:
                    break
                if e.get("followed"):
                    ev = dict(e, at=time.monotonic())
            # (none started: nothing the program imports is followed, the answer is the document)
            followed_ms = round((ev["at"] - t0) * 1000) if ev else -1 if r.get("following") else round(analyse_ms)
            followed_facts = len((ev or r)["result"].get("facts", []))
        p.stdin.write(json.dumps({"id": -1, "cmd": "shutdown"}) + "\n")
        p.stdin.flush()
        _, err = p.communicate()
        m = re.search(r"(\d+)\s+maximum resident set size", err)
        g = re.search(r"Maximum resident set size \(kbytes\): (\d+)", err)
        mb = int(m.group(1)) / 1e6 if m else int(g.group(1)) / 1e3 if g else None
        row = {"file": str(f.relative_to(root)), "function": qn, "line": line, "ok": r.get("ok"), "analyse_ms": round(analyse_ms), "on_demand": demand, "hover_ms": round(hover_ms), "total_ms": round(total), "peak_mb": mb and round(mb)}
        if followed_ms is not None:
            row.update(followed_ms=followed_ms, first_facts=len(r.get("result", {}).get("facts", [])), followed_facts=followed_facts)
        more = f", followed at {followed_ms} ms ({row['first_facts']} → {followed_facts} facts)" if followed_ms is not None else ""
        print(f"{row['file']}: analyse {row['analyse_ms']} ms{' (on demand)' if demand else ''}, hover {row['hover_ms']} ms, first hover at {row['total_ms']} ms{more}, {row['peak_mb']} MB", flush=True)
        rows.append(row)
    tot = [r["total_ms"] for r in rows]
    mem = [r["peak_mb"] or 0 for r in rows]
    summary = {"name": a.name, "modules": len(rows), "on_demand": sum(r["on_demand"] for r in rows), "first_hover_ms": {"p50": pct(tot, .5), "p95": pct(tot, .95), "max": max(tot, default=0)}, "peak_mb": max(mem, default=0)}
    fol = [r["followed_ms"] for r in rows if r.get("followed_ms") is not None]
    if fol:
        summary["followed_ms"] = {"p50": pct(fol, .5), "max": max(fol)}
    print(json.dumps(summary))
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / f"{a.name}-first.json").write_text(json.dumps({"summary": summary, "modules": rows}, indent=1))


if __name__ == "__main__":
    main()
