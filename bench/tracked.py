"""The tracked set: the 31 programs rowan is tuned on, measured one way.

  programs  the eight benchmarks (bench/prepare.py: stdlib12k, email, rich, click, pip, timm,
            sktime, functional), whole programs with drivers (stdlib12k: three stdlib modules
            in one file, no driver)
  modules   the cost set (bench/costset/manifest.json): 23 dense library modules of 500–7,000
            lines analysed as main files without drivers, as the held-out corpus is: parsers,
            lexers, serialisers, config, template, CLI and validation libraries, where rowan's
            work per line is highest. Picked by the manifest's `chosen`; a rotation of the
            held-out corpus (bench/heldout.py rotate --apply) adds its modules here.

Unlike the held-out corpus (bench/heldout.py), every program here may be profiled, read and
turned into repros. Cost-set modules come from three pinned places, each file's sha256 in the
manifest:

  stdlib   a module of the pinned CPython (.python-version), from the interpreter `uv run` uses,
           copied alone into bench/work/costset/stdlib (beside the stdlib's other files they
           would be the program's modules, not the library rowan models); .python-version pins
           the minor version only, so another patch release's text is measured too, marked
           `sha256_mismatch` in its run (CI's runners: 3.14.10)
  bench    a file of one of the eight benchmarks' programs (bench/prepare.py NAME), in place
  pypi     a module of a PyPI sdist (url, sha256), fetched into bench/work/costset/src, in place

    uv run --no-project python bench/tracked.py fetch [--only NAME,…]
    uv run --no-project python bench/tracked.py metrics [--rowan BIN] [--mode default|nodeadline|both]
            [--limits K=V,…] [--only NAME,…] [--shard K/N] [-j N] [--json OUT] [--profile DIR]
    uv run --no-project python bench/tracked.py table RESULT.json… [--vs BASE.json…] [--md]
    uv run --no-project python bench/tracked.py path NAME          the file rowan analyses

`metrics`: every program once per mode with bench/heldout.py's `run_module` (one `rowan FILE
--facts` with ROWAN_DEBUG=1 under .guard/run and /usr/bin/time: wall time, peak RSS, the `work:`
counters, whether the deadline cut it, verdict tiers, hover facts by level). Mode `default`: the
limits users run with (the 10 s deadline), cut at TIMEOUT_S["default"]; `nodeadline`:
ROWAN_LIMITS=ms=10^8, cut at TIMEOUT_S["nodeadline"] (the scoreboard's `tracked` family);
`both` runs each program in both modes before the next. One serial run (-j 1) is the timing
setting. `--shard K/N` measures every N-th program from the K-th (CI's matrix). `--profile DIR`
(Linux, `perf` on the PATH) then records each program once more, no deadline, under `perf record`
and writes DIR/NAME.profile.txt (top functions by self time) and, with inferno, a flamegraph.

`table`: one table of the results (shards merged): per program its kind (program / module),
lines, per mode wall time, peak RSS and cut / finished, the proven share of its verdicts and
the known share of its hover facts (no deadline), and the pooled lines (all 31, the programs,
the modules: bench/heldout.py's `pooled`). With `--vs`, each cell is `base → result`.
"""
from __future__ import annotations

import argparse
import functools
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import heldout  # noqa: E402

COSTSET = HERE / "costset" / "manifest.json"
WORK = HERE / "work" / "costset"
EIGHT = ["stdlib12k", "email", "rich", "click", "pip", "timm", "sktime", "functional"]
MODES = ("default", "nodeadline")
# per run: the default mode's 10 s deadline and what follows it; with no deadline the slowest
# cost-set modules take about a minute here (pyflakes' checker, pycparser's parser)
TIMEOUT_S = {"default": 60, "nodeadline": 180}


MISMATCHED: set[str] = set()  # stdlib modules whose text is not the manifest's here


def costset_manifest() -> dict:
    return json.loads(COSTSET.read_text())


def manifest_sha256() -> str:
    """What the set is: the cost set's manifest and prepare.py's pins (a change starts new series)."""
    h = hashlib.sha256(COSTSET.read_bytes())
    h.update((HERE / "prepare.py").read_bytes())
    return h.hexdigest()


def programs(only: str | None = None) -> list[dict]:
    """The 31: {name, kind: program | module, what, …}, the eight first."""
    out = [{"name": n, "kind": "program", "what": "benchmark"} for n in EIGHT]
    out += [{**m, "kind": "module", "what": m["kind"]} for m in costset_manifest()["modules"]]
    names = {p["name"] for p in out}
    want = only.split(",") if only else None
    if want and (bad := [w for w in want if w not in names]):
        sys.exit(f"tracked.py: no program {', '.join(bad)} (have {', '.join(sorted(names))})")
    return [p for p in out if not want or p["name"] in want]


def shard(progs: list[dict], spec: str | None) -> list[dict]:
    if not spec:
        return progs
    k, n = map(int, spec.split("/"))
    return progs[k::n]


@functools.cache
def stdlib_dir() -> Path:
    out = subprocess.run(["uv", "run", "--no-project", "python", "-c", "import sysconfig; print(sysconfig.get_paths()['stdlib'])"],
                         capture_output=True, text=True, check=True, cwd=HERE.parent)
    return Path(out.stdout.strip())


def bench_dir(name: str) -> Path:
    d = HERE / "work" / name
    if not d.exists() or not any(d.glob("*.py")):
        subprocess.run(["uv", "run", "--no-project", "python", str(HERE / "prepare.py"), name], check=True, stdout=subprocess.DEVNULL)
    return d


def path_of(p: dict) -> Path:
    """The file rowan analyses: a benchmark's main file; a cost-set module's file, a package's
    in place (its other modules beside it, as Python finds them), a stdlib module copied alone."""
    if p["kind"] == "program":
        d = bench_dir(p["name"])
        # (stdlib12k: prepare.py's one file; hover_setup.py writes _all_modules.py beside it)
        return d / "big.py" if p["name"] == "stdlib12k" else d / "main.py"
    if p["source"] == "pypi":
        pkgs = {q["name"]: q for q in costset_manifest()["packages"]}
        saved, heldout.SRC = heldout.SRC, WORK / "src"
        try:
            src = heldout.fetch_one(pkgs[p["package"]]) / p["path"]
        finally:
            heldout.SRC = saved
    elif p["source"] == "stdlib":
        src = stdlib_dir() / p["path"]
        dst = WORK / "stdlib" / (p["name"] + ".py")
        if not dst.exists() or dst.read_bytes() != src.read_bytes():
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes(src.read_bytes())
        src = dst
    else:
        src = bench_dir(p["bench"]) / p["path"]
    if hashlib.sha256(src.read_bytes()).hexdigest() != p["sha256"]:
        if p["source"] != "stdlib":
            sys.exit(f"{p['name']}: sha256 mismatch for {src}")
        # (the interpreter's own patch release: .python-version pins 3.14, not its patch, so a
        # stdlib module, as stdlib12k, may differ between machines; measured, and marked)
        MISMATCHED.add(p["name"])
        print(f"tracked.py: {p['name']}: {src} is not the manifest's (another 3.14 patch release); measured anyway", file=sys.stderr)
    return src


def lines_of(p: dict, path: Path) -> int:
    """A module's lines; a program's, all of its directory's Python (what prepare.py prints)."""
    if p["kind"] == "module" or p["name"] == "stdlib12k":
        return path.read_text(errors="replace").count("\n")
    return sum(f.read_text(errors="replace").count("\n") for f in path.parent.rglob("*.py"))


def limits_for(mode: str, limits: str | None) -> str | None:
    return ",".join(x for x in [limits, "ms=100000000"] if x) if mode == "nodeadline" else limits


def run_one(binary: Path, p: dict, path: Path, mode: str, limits: str | None, out_dir: Path) -> dict:
    """One measured run of one program in one mode (bench/heldout.py's `run_module`)."""
    r = heldout.run_module(Path(binary), path, limits_for(mode, limits), TIMEOUT_S[mode], out_dir / mode)
    r.update(kind=p["kind"], what=p["what"])
    if p["name"] in MISMATCHED:
        r["sha256_mismatch"] = True
    return r


def show(name: str, mode: str, r: dict) -> None:
    print(f"  {name:<22} {mode:<10} {'ok ' if r['finished'] else r.get('why')} {str(r.get('wall_s')):>6} s"
          f"{' cut' if r.get('cut') else '    '} {str(r.get('rss_mb')):>5} MB  work {r.get('work')}", file=sys.stderr, flush=True)


def metrics(binary: Path, mode: str, limits: str | None, only: str | None, jobs: int, out_dir: Path, shard_spec: str | None = None) -> dict:
    """Every program (of the shard) in each mode; `mode` is one of MODES or `both`. Returns
    {set, binary, sha256, modes: {mode: {limits, timeout_s, pooled, by_kind, runs}}} (for one
    mode also that mode's fields at the top, as bench/heldout.py's `metrics` returns them)."""
    from concurrent.futures import ThreadPoolExecutor

    modes = MODES if mode == "both" else (mode,)
    todo = [(p, path_of(p)) for p in shard(programs(only), shard_spec)]
    lines = {p["name"]: lines_of(p, path) for p, path in todo}

    def one(t):
        p, path = t
        out = {}
        for m in modes:
            r = run_one(binary, p, path, m, limits, out_dir)
            r["lines"] = lines[p["name"]]
            show(p["name"], m, r)
            out[m] = r
        return p["name"], out

    with ThreadPoolExecutor(jobs) as ex:
        runs = dict(ex.map(one, todo))
    res = {"set": "tracked", "binary": str(binary), "sha256": manifest_sha256(), "modes": {}}
    for m in modes:
        rs = {n: r[m] for n, r in runs.items()}
        res["modes"][m] = {"limits": limits_for(m, limits), "timeout_s": TIMEOUT_S[m], **pool(rs), "runs": rs}
    if len(modes) == 1:
        res.update(res["modes"][modes[0]], corpus="tracked", mode=modes[0])
    return res


def pool(runs: dict) -> dict:
    by_kind = {k: heldout.pooled({n: r for n, r in runs.items() if r["kind"] == k}) for k in ("program", "module") if any(r["kind"] == k for r in runs.values())}
    return {"pooled": heldout.pooled(runs), "by_kind": by_kind}


def profile(binary: Path, only: str | None, shard_spec: str | None, out: Path) -> None:
    """Each program once more under `perf record` (no deadline): top functions by self time,
    and a flamegraph where inferno is installed. Linux only; nothing elsewhere."""
    if not shutil.which("perf"):
        print("tracked.py: no perf, no profiles", file=sys.stderr)
        return
    out.mkdir(parents=True, exist_ok=True)
    for p in shard(programs(only), shard_spec):
        path = path_of(p)
        data = out / f"{p['name']}.perf.data"
        env = dict(os.environ, ROWAN_LIMITS="ms=100000000")
        try:
            subprocess.run(["perf", "record", "-F", "199", "-g", "-o", str(data), "--", str(binary), str(path), "--facts"], cwd=path.parent, env=env,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=TIMEOUT_S["nodeadline"])
        except subprocess.TimeoutExpired:
            pass
        rep = subprocess.run(["perf", "report", "-i", str(data), "--stdio", "--no-children", "--sort", "symbol", "-g", "none", "--percent-limit", "0.5"],
                             capture_output=True, text=True)
        top = [ln for ln in rep.stdout.splitlines() if ln.strip() and not ln.startswith("#")][:40]
        (out / f"{p['name']}.profile.txt").write_text("\n".join(top) + "\n")
        if shutil.which("inferno-collapse-perf") and shutil.which("inferno-flamegraph"):
            subprocess.run(f"perf script -i '{data}' 2>/dev/null | inferno-collapse-perf | inferno-flamegraph --title 'rowan: {p['name']}' > '{out / (p['name'] + '.flamegraph.svg')}'",
                           shell=True, check=False)
        data.unlink(missing_ok=True)
        print(f"  {p['name']}: profiled", file=sys.stderr, flush=True)


# ---------------------------------------------------------------- the table
def merged(files: list[str]) -> dict:
    """Results of several runs (CI's shards) as one: {mode: {name: run}}."""
    out: dict = {}
    for f in files:
        doc = json.loads(Path(f).read_text())
        for m, res in doc.get("modes", {}).items():
            out.setdefault(m, {}).update(res["runs"])
    return out


def fmt(x, d=1, unit="") -> str:
    return "–" if x is None else f"{x:,.{d}f}{unit}"


def share(r: dict, what: str) -> float | None:
    if not r or not r.get("finished"):
        return None
    if what == "proven":
        t = r.get("tiers") or {}
        n = sum(t.values())
        return 100 * t.get("proven", 0) / n if n else None
    f = r.get("facts") or {}
    n = f.get("unknown", 0) + f.get("partial", 0) + f.get("exact", 0)
    return 100 * (f.get("partial", 0) + f.get("exact", 0)) / n if n else None


def cell(a, b, fmt_one) -> str:
    """`b`, or `a → b` with a base."""
    return fmt_one(b) if a is False else f"{fmt_one(a)} → {fmt_one(b)}"


def run_cells(r: dict | None, mode: str) -> tuple:
    if not r:
        return (None, None, "missing")
    state = "ok" if r.get("finished") and not r.get("cut") else "cut" if r.get("finished") else ("timeout" if "over" in str(r.get("why", "")) else "failed")
    return (r.get("wall_s"), r.get("rss_mb") if r.get("finished") else None, state)


def table(res: dict, base: dict | None = None, md: bool = False) -> str:
    """One table: a row per program, then the pooled rows (`res`, `base`: merged())."""
    names = [p["name"] for p in programs()]
    kinds = {p["name"]: p["kind"] for p in programs()}
    modes = [m for m in MODES if m in res]
    head = ["program", "kind", "lines"]
    for m in modes:
        tag = "10 s deadline" if m == "default" else "no deadline"
        head += [f"wall ({tag})", f"RSS ({tag})", f"state ({tag})"]
    pm = "nodeadline" if "nodeadline" in res else modes[0]
    head += ["proven %", "known facts %"]
    rows = []
    for n in names:
        if not any(n in res[m] for m in modes):
            continue
        any_r = next(res[m][n] for m in modes if n in res[m])
        row = [n, kinds[n], f"{any_r.get('lines', 0):,}"]
        for m in modes:
            w, s, st = run_cells(res[m].get(n), m)
            if base is not None:
                bw, bs, bst = run_cells((base.get(m) or {}).get(n), m)
                row += [f"{fmt(bw, 2)} → {fmt(w, 2)} s", f"{fmt(bs, 0)} → {fmt(s, 0)} MB", st if st == bst else f"{bst} → {st}"]
            else:
                row += [fmt(w, 2, " s"), fmt(s, 0, " MB"), st]
        for what in ("proven", "known"):
            v = share(res[pm].get(n), what)
            row.append(f"{fmt(share((base.get(pm) or {}).get(n), what))} → {fmt(v)}" if base is not None else fmt(v))
        rows.append(row)
    # the pooled rows
    for label, keep in (("all", lambda k: True), ("programs", lambda k: k == "program"), ("modules", lambda k: k == "module")):
        row = [f"pooled: {label}", "", ""]
        pooled_now = {m: heldout.pooled({n: r for n, r in res[m].items() if keep(r.get("kind", kinds.get(n)))}) for m in modes}
        pooled_base = {m: heldout.pooled({n: r for n, r in (base.get(m) or {}).items() if keep(r.get("kind", kinds.get(n)))}) for m in modes} if base is not None else None
        if not pooled_now[modes[0]]["modules"]:
            continue
        for m in modes:
            p = pooled_now[m]
            cur = (p["wall_geomean_s"], p["rss_geomean_mb"], f"{p['finished']}/{p['modules']} fin, {p['cut']} cut")
            if pooled_base:
                b = pooled_base[m]
                row += [f"{fmt(b['wall_geomean_s'], 2)} → {fmt(cur[0], 2)} s gm", f"{fmt(b['rss_geomean_mb'], 0)} → {fmt(cur[1], 0)} MB gm",
                        f"{b['finished']}/{b['modules']}, {b['cut']} cut → {cur[2]}"]
            else:
                row += [fmt(cur[0], 2, " s gm"), fmt(cur[1], 0, " MB gm"), cur[2]]
        p = pooled_now[pm]
        if pooled_base:
            b = pooled_base[pm]
            row += [f"{fmt(b['proven_pct'])} → {fmt(p['proven_pct'])}", f"{fmt(b['known_facts_pct'])} → {fmt(p['known_facts_pct'])}"]
        else:
            row += [fmt(p["proven_pct"]), fmt(p["known_facts_pct"])]
        rows.append(row)
    if md:
        out = ["| " + " | ".join(head) + " |", "| " + " | ".join("---" for _ in head) + " |"]
        out += ["| " + " | ".join(r) + " |" for r in rows]
        return "\n".join(out) + "\n"
    w = [max(len(str(r[i])) for r in [head, *rows]) for i in range(len(head))]
    return "\n".join("  ".join(str(c).ljust(w[i]) for i, c in enumerate(r)) for r in [head, *rows]) + "\n"


# ---------------------------------------------------------------- commands
def cmd_fetch(a) -> None:
    for p in programs(a.only):
        path = path_of(p)
        print(f"{p['name']} ({p['kind']}): {lines_of(p, path):,} lines, {path}")


def cmd_metrics(a) -> None:
    binary = Path(a.rowan).resolve()
    res = metrics(binary, a.mode, a.limits, a.only, a.j, WORK / "logs", a.shard)
    if a.json:
        Path(a.json).parent.mkdir(parents=True, exist_ok=True)
        Path(a.json).write_text(json.dumps(res, indent=1))
    for m, r in res["modes"].items():
        heldout.print_pooled(f"tracked {m}", r["pooled"])
        for k, s in r["by_kind"].items():
            heldout.print_pooled(f"  {k}s", s)
    print(table({m: r["runs"] for m, r in res["modes"].items()}))
    if a.profile:
        profile(binary, a.only, a.shard, Path(a.profile))


def cmd_table(a) -> None:
    print(table(merged(a.results), merged(a.vs) if a.vs else None, a.md), end="")


def cmd_path(a) -> None:
    print(path_of(programs(a.name)[0]))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    q = sub.add_parser("fetch")
    q.add_argument("--only")
    q.set_defaults(fn=cmd_fetch)
    q = sub.add_parser("metrics")
    q.add_argument("--rowan", default=str(heldout.RUST_BIN))
    q.add_argument("--mode", default="both", choices=[*MODES, "both"])
    q.add_argument("--limits")
    q.add_argument("--only")
    q.add_argument("--shard", help="K/N: every N-th program from the K-th")
    q.add_argument("--json")
    q.add_argument("--profile", help="then perf profiles into this directory (Linux)")
    q.add_argument("-j", type=int, default=1)
    q.set_defaults(fn=cmd_metrics)
    q = sub.add_parser("table")
    q.add_argument("results", nargs="+")
    q.add_argument("--vs", nargs="+")
    q.add_argument("--md", action="store_true")
    q.set_defaults(fn=cmd_table)
    q = sub.add_parser("path")
    q.add_argument("name")
    q.set_defaults(fn=cmd_path)
    a = ap.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
