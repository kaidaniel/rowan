"""The cost set: dense library modules rowan is tuned on for per-module cost (bench/costset/manifest.json).

Single modules of 500–6,000 lines analysed as main files without drivers, as stdlib12k and the
held-out corpus are: parsers, lexers, serialisers, config, template, CLI and validation
libraries, where rowan's work per line is highest. Unlike the held-out corpus
(bench/heldout.py), this set may be profiled, read and turned into repros. Its modules come from
three pinned places, each file's sha256 in the manifest:

  stdlib   a module of the pinned CPython (.python-version), from the interpreter `uv run` uses,
           copied alone into bench/work/costset/stdlib (beside the stdlib's other files they
           would be the program's modules, not the library rowan models)
  bench    a file of one of the eight benchmarks' programs (bench/prepare.py NAME, pinned there),
           analysed in place
  pypi     a module of a PyPI sdist (url, sha256), fetched into bench/work/costset/src, in place

The manifest's `chosen` says how the 23 were picked from 85 candidates (none in the held-out
manifest): those the 10 s deadline cut or over 500 MB at v0.1.83, and the slowest others.

    uv run --no-project python bench/costset.py fetch
    uv run --no-project python bench/costset.py metrics [--rowan BIN] [--mode default|nodeadline]
            [--limits K=V,…] [--only NAME,…] [-j N] [--json OUT]

`metrics` is bench/heldout.py's: per module one `rowan MODULE --facts` with ROWAN_DEBUG=1 under
.guard/run and `/usr/bin/time -l`: wall time, peak RSS, the `work:` counters, whether the
deadline cut it, verdict tiers and hover facts, pooled as there; and a line per module.
"""
from __future__ import annotations

import argparse
import functools
import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import heldout  # noqa: E402

MANIFEST = HERE / "costset" / "manifest.json"
WORK = HERE / "work" / "costset"


def manifest() -> dict:
    return json.loads(MANIFEST.read_text())


@functools.cache
def stdlib_dir() -> Path:
    out = subprocess.run(["uv", "run", "--no-project", "python", "-c", "import sysconfig; print(sysconfig.get_paths()['stdlib'])"],
                         capture_output=True, text=True, check=True, cwd=HERE.parent)
    return Path(out.stdout.strip())


def bench_dir(name: str) -> Path:
    d = HERE / "work" / name
    if not d.exists():
        subprocess.run(["uv", "run", "--no-project", "python", str(HERE / "prepare.py"), name], check=True, stdout=subprocess.DEVNULL)
    return d


def module_path(m: dict, pkgs: dict) -> Path:
    """The module's file: a package's in place (its other modules beside it, as Python finds
    them), a stdlib module copied alone."""
    if m["source"] == "pypi":
        src = heldout.fetch_one(pkgs[m["package"]]) / m["path"]
    elif m["source"] == "stdlib":
        # (alone, as stdlib12k: beside the stdlib's other files they would be the program's
        # modules, not the library rowan models)
        src = stdlib_dir() / m["path"]
        dst = WORK / "stdlib" / (m["name"] + ".py")
        if not dst.exists() or dst.read_bytes() != src.read_bytes():
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes(src.read_bytes())
        src = dst
    else:
        src = bench_dir(m["bench"]) / m["path"]
    if hashlib.sha256(src.read_bytes()).hexdigest() != m["sha256"]:
        sys.exit(f"{m['name']}: sha256 mismatch for {src}")
    return src


def selected(only: str | None) -> tuple[list[dict], dict]:
    man = manifest()
    pkgs = {p["name"]: p for p in man["packages"]}
    mods = [m for m in man["modules"] if not only or m["name"] in only.split(",")]
    return mods, pkgs


def cmd_fetch(a) -> None:
    heldout.SRC = WORK / "src"
    mods, pkgs = selected(a.only)
    for m in mods:
        p = module_path(m, pkgs)
        print(f"{m['name']}: {p.read_text(errors='replace').count(chr(10))} lines")


def metrics(binary: Path, mode: str, limits: str | None, only: str | None, jobs: int, out_dir: Path) -> dict:
    """Every module once (bench/heldout.py's `run_module`), pooled as there: the scoreboard's
    `costset` family runs it with mode `nodeadline`, serially."""
    from concurrent.futures import ThreadPoolExecutor

    saved, heldout.SRC = heldout.SRC, WORK / "src"
    mods, pkgs = selected(only)
    lim, timeout = limits, heldout.DEFAULT_TIMEOUT_S
    if mode == "nodeadline":
        lim = ",".join(x for x in [limits, "ms=100000000"] if x)
        timeout = heldout.NODEADLINE_TIMEOUT_S
    todo = [(m, module_path(m, pkgs)) for m in mods]

    def one(t):
        m, path = t
        r = heldout.run_module(Path(binary), path, lim, timeout, out_dir)
        r["kind"] = m["kind"]
        r["lines"] = path.read_text(errors="replace").count("\n")
        print(f"  {m['name']:<28} {r['lines']:>5} lines  {'ok ' if r['finished'] else r.get('why')} {str(r.get('wall_s')):>6} s"
              f"{' cut' if r.get('cut') else '    '} {str(r.get('rss_mb')):>5} MB  work {r.get('work')}", file=sys.stderr, flush=True)
        return m["name"], r

    with ThreadPoolExecutor(jobs) as ex:
        runs = dict(ex.map(one, todo))
    # (the held-out corpus's sources where its own runs, e.g. the scoreboard's next family, find them)
    heldout.SRC = saved
    return {"corpus": "costset", "mode": mode, "limits": lim, "binary": str(binary), "pooled": heldout.pooled(runs), "runs": runs}


def cmd_metrics(a) -> None:
    res = metrics(Path(a.rowan), a.mode, a.limits, a.only, a.j, WORK / "logs" / a.mode)
    heldout.print_pooled("costset", res["pooled"])
    if a.json:
        Path(a.json).write_text(json.dumps(res, indent=1))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name, fn in (("fetch", cmd_fetch), ("metrics", cmd_metrics)):
        q = sub.add_parser(name)
        q.set_defaults(fn=fn)
        q.add_argument("--only")
        if name == "metrics":
            q.add_argument("--rowan", default=str(heldout.RUST_BIN))
            q.add_argument("--mode", default="default", choices=["default", "nodeadline"])
            q.add_argument("--limits")
            q.add_argument("--json")
            q.add_argument("-j", type=int, default=1)
    a = ap.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
