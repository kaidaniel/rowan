"""Time to information in `rowan serve`: the whole program's analysis of a file vs one hover.

Usage: python bench/serve_hovers.py ROWAN NAME [--modules K] [--per-module N] [--whole] [--out DIR]

For a benchmark program (bench/work/NAME, see prepare.py) it picks K modules of the package
spread over its files, and in each up to N functions (module-level defs and methods of
module-level classes, spread over the file). For each module it starts a `rowan serve`, asks
for a hover inside each function (`{"cmd":"hover"}`: the module's own code is set up first,
then the function runs for any call) and asks again (the cached answer). With --whole it also
times the whole program's analysis of the module (`rowan FILE --facts`, no deadline).

Prints the setup time per module and p50 / p95 / max of the hover times, cold and cached,
with the statements each hover ran; writes DIR/NAME-hover.json with every measurement.
"""

import argparse
import ast
import json
import os
import subprocess
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
GUARD = HERE.parent / ".guard" / "run"


def guarded(t: int) -> list[str]:
    """The guard's prefix on the owner's Mac (.guard/run, not in the repository); none elsewhere (CI)."""
    return [str(GUARD), "-t", str(t)] if GUARD.exists() else []


def functions(path: Path, n: int) -> list[tuple[str, int]]:
    """Up to n functions of the file (module-level defs, methods of module-level classes),
    spread over it: (qualified name, a line inside it)."""
    try:
        tree = ast.parse(path.read_text())
    except (SyntaxError, UnicodeDecodeError):
        return []
    out = []
    for s in tree.body:
        defs = [s] if isinstance(s, (ast.FunctionDef, ast.AsyncFunctionDef)) else []
        if isinstance(s, ast.ClassDef):
            defs = [(m, s.name) for m in s.body if isinstance(m, (ast.FunctionDef, ast.AsyncFunctionDef))]
        for d in defs:
            d, cls = d if isinstance(d, tuple) else (d, None)
            out.append((f"{cls}.{d.name}" if cls else d.name, d.body[0].lineno))
    if len(out) > n:
        step = len(out) / n
        out = [out[int(i * step)] for i in range(n)]
    return out


def pct(xs: list[float], p: float) -> float:
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(p * (len(xs) - 1))))] if xs else 0.0


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("rowan")
    ap.add_argument("name")
    ap.add_argument("--modules", type=int, default=20)
    ap.add_argument("--per-module", type=int, default=15)
    ap.add_argument("--whole", action="store_true")
    ap.add_argument("--out", default="bench-results")
    a = ap.parse_args()
    root = HERE / "work" / a.name
    files = sorted(p for p in root.rglob("*.py") if not p.name.startswith("_imports_only") and p.name != "main.py")
    files = [f for f in files if len(functions(f, 1)) > 0]
    if len(files) > a.modules:
        step = len(files) / a.modules
        files = [files[int(i * step)] for i in range(a.modules)]
    rows = []
    for f in files:
        fns = functions(f, a.per_module)
        env = dict(os.environ)
        env.pop("ROWAN_DEBUG", None)
        p = subprocess.Popen(guarded(0) + [a.rowan, "serve"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env)
        setup_ms = None
        mod = {"file": str(f.relative_to(root)), "hovers": []}
        for i, (qn, line) in enumerate(fns):
            for again in (False, True):
                p.stdin.write(json.dumps({"id": i, "cmd": "hover", "path": str(f), "line": line}) + "\n")
                p.stdin.flush()
                r = json.loads(p.stdout.readline())
                if not r.get("ok"):
                    mod["hovers"].append({"function": qn, "line": line, "error": r.get("error")})
                    break
                if r.get("setup_ms") is not None:
                    setup_ms = r["setup_ms"]
                h = {"function": qn, "line": line, "cached": r["cached"], "ms": r["ms"], "span": r.get("span")}
                if not r["cached"]:
                    h.update(r["result"].get("work", {}))
                h["facts"] = len(r["result"].get("facts", []))
                mod["hovers"].append(h)
        p.stdin.write(json.dumps({"id": -1, "cmd": "shutdown"}) + "\n")
        p.stdin.flush()
        p.wait()
        mod["setup_ms"] = setup_ms
        if a.whole:
            t0 = time.monotonic()
            w = subprocess.run(guarded(600) + [a.rowan, str(f), "--facts"], capture_output=True, text=True, env=dict(env, ROWAN_LIMITS="ms=100000000"))
            mod["whole_s"] = round(time.monotonic() - t0, 2)
            mod["whole_ok"] = w.returncode == 0
        cold = [h["ms"] for h in mod["hovers"] if "ms" in h and not h["cached"]]
        print(f"{mod['file']}: setup {setup_ms} ms, {len(cold)} hovers p50 {pct(cold, .5):.0f} max {max(cold, default=0):.0f} ms" + (f", whole {mod['whole_s']} s" if a.whole else ""), flush=True)
        rows.append(mod)
    cold = [h["ms"] for m in rows for h in m["hovers"] if "ms" in h and not h["cached"]]
    cached = [h["ms"] for m in rows for h in m["hovers"] if "ms" in h and h["cached"]]
    setup = [m["setup_ms"] for m in rows if m["setup_ms"] is not None]
    errors = sum(1 for m in rows for h in m["hovers"] if "error" in h)
    summary = {
        "name": a.name, "modules": len(rows), "hovers": len(cold), "errors": errors,
        "setup_ms": {"p50": pct(setup, .5), "p95": pct(setup, .95), "max": max(setup, default=0)},
        "hover_ms": {"p50": pct(cold, .5), "p95": pct(cold, .95), "max": max(cold, default=0)},
        "cached_ms": {"p50": pct(cached, .5), "p95": pct(cached, .95), "max": max(cached, default=0)},
    }
    if a.whole:
        whole = [m["whole_s"] for m in rows]
        summary["whole_s"] = {"p50": pct(whole, .5), "p95": pct(whole, .95), "max": max(whole, default=0)}
    print(json.dumps(summary))
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / f"{a.name}-hover.json").write_text(json.dumps({"summary": summary, "modules": rows}, indent=1))


if __name__ == "__main__":
    main()
