"""The held-out corpus: real Python that no iteration tunes rowan against (bench/heldout/manifest.json).

31 pure-Python packages from PyPI, none of them in the eight benchmarks, the site-packages
scenarios or the golden suites, pinned (the latest sdist before the manifest's `snapshot` date,
sha256 checked) and analysed without drivers: each package's two largest modules (tests, docs,
examples left out; one exception, said in its `why`) are run as the main file, as stdlib12k is,
so rowan's entry runs do the work and nobody writes code for rowan to read. Agents report these
numbers; they do not pick work from its individual sites (AGENTS.md, SUBAGENTS.md).

    uv run --no-project python bench/heldout.py fetch
    uv run --no-project python bench/heldout.py metrics [--rowan BIN] [--mode default|nodeadline]
            [--corpus heldout|tracked] [--limits K=V,…] [--only PKG,…] [-j N] [--json OUT]
    uv run --no-project python bench/heldout.py sound [--rowan BIN] [--only PKG,…] [--json OUT] [-v]
            [--baseline FILE] [--report OUT.md] [--write-baseline FILE] [--scratch DIR]
    uv run --no-project python bench/heldout.py sweep [--rowan BIN] [--bins NAME=BIN,…] [--variants …]
            [--corpus heldout|tracked] [--mode …] [--json OUT]

`metrics`: per module one `rowan MODULE --facts` with ROWAN_DEBUG=1 (`/usr/bin/time -l` under
.guard/run), as the scoreboard measures the eight: verdict tiers, the main file's hover facts by
level (bench/sitepackages.py's `level`), wall time, peak RSS, the `work:` counters, whether the
deadline cut it (`cut`) and how many calls ran past their budget (`skipped calls`). Mode
`default`: the limits users run with (the 10 s deadline); `nodeadline`: ROWAN_LIMITS=ms=10^8, cut
at NODEADLINE_TIMEOUT_S (the scoreboard's setting for the eight). Pooled like the scoreboard: proven
% and possibly % of all verdicts, known facts % = (partial + exact) / (unknown + partial + exact).

`sound`: each package's own pytest suite (in a uv venv under bench/work/heldout/venv) against
rowan's claims on its analysed modules (claims from `--golden` with ROWAN_LIMITS=CLAIM_LIMITS, so
that they do not depend on the machine's load; a module rowan fails on is counted, not compared).
What is compared:
  * values: every simple assignment (`x = …`, `a, b = …`, `x: T = …`, `x += …`) on lines of its
    own (no yield / await) and a function's parameters after its first statement get a
    `#@general NAME` directive in a copy of the module (same lines), so `--golden` reports rowan's
    value and record keys there (`lits`, `unknown`, `objects`, `types`, the joined row). Inside a
    function only the runs *for any call* count (rows marked `general`: hover values; what the
    module's own calls give holds only for those calls); at module level every run (import-time
    code runs as when imported). A test run's value is outside the claim when: a
    scalar (None, bool, int, float, str, bytes) whose repr is not among the literals and the claim
    has no unknown admitting its type (`int(?)` admits a bool); anything else where the claim has
    only literals; a dict whose keys miss a `definite` key, hold an `absent` one, or leave a
    `closed` record's `present` ones; a package instance's `__dict__` holding an `absent` attribute
    or leaving a closed one's (not `definite`: rowan's attributes include the class's); only where
    all the claim's objects are dicts, or none is (the row is joined over them). Class bodies are
    not watched (the golden document records no checks there).
  * lines reached: a watched statement the tests complete where the claim has no run reaching it
    although the function had a run for any call (rowan took it for unreachable).
  * access outcomes: reads with a `proven` / `missing` verdict reached without a call (module code,
    a function's own entry run: a chain of at most its `def` line), unguarded, all of one tier at
    the line and key: a KeyError / AttributeError for the key raised in that frame at a `proven`
    line, or a `missing` line that ran without one.
A mismatch is a candidate: each is either an unsound claim (shrink it into a repro) or a limit of
this matching (tests calling with types the annotations exclude, monkeypatching, mocks); the
report says which. `--baseline`: the candidate sites compared with the known ones
(heldout/sound_baseline.json: identifiers, each with the kind that explains it), exit 1 when
there are new ones (CI: heldout.yml, after every merge). Sites name modules by their path in
the sdist, wherever the corpus lies (a scratch worktree's bench/work linked to the main
checkout's gives the same sites). `--scratch DIR`: the annotated copies, observations and test
points go there instead of bench/work/heldout (two `sound` runs at once, as bench/all.py's).

`--corpus tracked`: the tracked set (bench/tracked.py: the eight benchmarks and the cost set)
measured the same way, for the tuned-on side of the comparison.

`sweep`: `metrics` at each knob ×½ and ×2: ROWAN_LIMITS budget and ms, and binaries built with
other constants (`--bins`).

`rotate` (the owner's, about every 20 releases; AGENTS.md, Tests):
    uv run --no-project python bench/heldout.py rotate --draw [--candidates FILE|URL] [--snapshot DATE]
            [-n 31] [--seed S] [--out PROPOSAL.json]
    uv run --no-project python bench/heldout.py rotate --apply PROPOSAL.json
`--draw` proposes a fresh corpus by #86's rules: candidates (names, one per line; default the
top PyPI packages of the last 30 days) shuffled by the seed and taken in turn while they pass:
pure Python (every wheel `none-any`, no C / Cython / Rust source in the sdist), no overlap (by
distribution and import name) with the eight, the cost set, the site-packages scenarios, the
golden suites' imports or the current corpus, the latest sdist uploaded before the snapshot date
(sha256 kept), its two largest modules (tests, docs, examples left out) 200–8,000 lines
together, its tests where the sdist has them (pytest and its runtime requirements). It writes a
proposal and changes nothing: the owner reviews it (kinds, whys, test arguments). `--apply` then
retires the current corpus into the cost set's manifest (tuned on from then on: source `pypi`,
each module's sha256), installs the proposal as the manifest and resets sound_baseline.json (its
kinds kept, no sites; then `sound --write-baseline` and classify).
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import io
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tarfile
import time
import random
import urllib.request
import zipfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
GUARD = ROOT / ".guard" / "run"
# (the guard is for the owner's Mac (AGENTS.md); elsewhere, CI's runners, a run is cut by its
# timeout here and timed by GNU time)
DARWIN = sys.platform == "darwin"
TIME = ["/usr/bin/time", "-l"] if DARWIN else ["/usr/bin/time", "-v"]
MANIFEST = HERE / "heldout" / "manifest.json"
BASELINE = HERE / "heldout" / "sound_baseline.json"
WORK = HERE / "work" / "heldout"
SRC = WORK / "src"
SCRATCH = WORK  # sound's annotated copies, observations, test points (`--scratch`)
RUST_BIN = ROOT / "rowan" / "target" / "release" / "rowan"
DEFAULT_TIMEOUT_S = 60  # a default-mode run (the 10 s deadline, plus what follows it)
NODEADLINE_TIMEOUT_S = 60
TEST_TIMEOUT_S = 900
CLAIM_LIMITS = "ms=60000"
CLAIM_TIMEOUT_S = 150
sys.path.insert(0, str(HERE))


def manifest() -> list[dict]:
    return json.loads(MANIFEST.read_text())["packages"]


def select(pkgs: list[dict], only: str | None) -> list[dict]:
    return [p for p in pkgs if not only or p["name"] in only.split(",")]


# ---------------------------------------------------------------- fetch
def src_dir(p: dict) -> Path:
    return SRC / f"{p['name']}-{p['version']}"


def fetch_one(p: dict) -> Path:
    """The sdist, sha256 checked against the manifest, extracted under bench/work/heldout/src
    (its single top directory renamed NAME-VERSION)."""
    d = src_dir(p)
    if (d / ".stamp").exists() and (d / ".stamp").read_text() == p["sha256"]:
        return d
    shutil.rmtree(d, ignore_errors=True)
    for attempt in range(5):
        try:
            data = urllib.request.urlopen(p["url"], timeout=60).read()
            break
        except OSError:
            if attempt == 4:
                raise
            time.sleep(2 ** attempt)
    if hashlib.sha256(data).hexdigest() != p["sha256"]:
        sys.exit(f"{p['name']} {p['version']}: sha256 mismatch")
    tmp = SRC / f".tmp-{p['name']}"
    shutil.rmtree(tmp, ignore_errors=True)
    tmp.mkdir(parents=True)
    with tarfile.open(fileobj=io.BytesIO(data)) as t:
        t.extractall(tmp, filter="data")
    (top,) = list(tmp.iterdir())
    top.rename(d)
    tmp.rmdir()
    (d / ".stamp").write_text(p["sha256"])
    return d


def cmd_fetch(a) -> None:
    for p in select(manifest(), a.only):
        d = fetch_one(p)
        lines = sum((d / m).read_text(errors="replace").count("\n") for m in p["modules"])
        print(f"{p['name']}=={p['version']}: {', '.join(p['modules'])} ({lines} lines)")


# ---------------------------------------------------------------- metrics
WORK_FIELDS = {"work": r"^work: (\d+)", "units": r"units (\d+)", "tabulations": r"tabulations (\d+)", "objects": r"objects (\d+)",
               "row_joins": r"row joins (\d+)", "havoc": r"havoc visits (\d+)", "skipped_calls": r"skipped calls (\d+)"}


def run_guarded(cmd: list[str], timeout: int, **kw) -> tuple[int, bool]:
    """`cmd` under the guard (macOS) or cut at `timeout` with its process group: (exit code,
    whether the time limit cut it)."""
    if DARWIN:
        p = subprocess.run([str(GUARD), "-t", str(timeout), *cmd], stdin=subprocess.DEVNULL, **kw)
        return p.returncode, p.returncode == 124
    p = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, start_new_session=True, **kw)
    try:
        return p.wait(timeout=timeout), False
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, 9)
        p.wait()
        return 124, True


def run_module(binary: Path, path: Path, limits: str | None, timeout: int, out_dir: Path) -> dict:
    """One `rowan PATH --facts` run, timed: tiers, fact levels, time, memory, counters."""
    from scoreboard import facts_summary

    out_dir.mkdir(parents=True, exist_ok=True)
    tag = re.sub(r"\W", "_", str(path.relative_to(SRC) if path.is_relative_to(SRC) else path.relative_to(HERE) if path.is_relative_to(HERE) else path))
    out, err = out_dir / f"{tag}.facts.json", out_dir / f"{tag}.err"
    env = dict(os.environ, ROWAN_DEBUG="1")
    env.pop("ROWAN_LIMITS", None)
    if limits:
        env["ROWAN_LIMITS"] = limits
    t0 = time.monotonic()
    with open(out, "w") as so, open(err, "w") as se:
        rc, over = run_guarded([*TIME, str(binary), str(path), "--facts"], timeout, stdout=so, stderr=se, env=env, cwd=path.parent)
    r = {"finished": rc == 0, "wall_s": round(time.monotonic() - t0, 2)}
    if over:
        r["why"] = f"over {timeout} s"
    for line in err.read_text(errors="replace").splitlines():
        if line.startswith("work: "):
            for k, pat in WORK_FIELDS.items():
                if m := re.search(pat, line):
                    r[k] = int(m.group(1))
        elif m := re.match(r"^\s*([\d.]+) real", line):
            r["wall_s"] = float(m.group(1))
        elif "maximum resident set size" in line:
            r["rss_mb"] = round(int(line.split()[0]) / 2**20)
        elif "Maximum resident set size (kbytes)" in line:
            r["rss_mb"] = round(int(line.split()[-1]) / 2**10)
        elif line.startswith("GUARD"):
            r["why"] = line.strip()
    if not r["finished"]:
        r.setdefault("why", f"exit {rc}")
        if "over" in r["why"]:
            r["wall_s"] = timeout
    else:
        try:
            doc = json.loads(out.read_text())
            r.update(facts_summary(doc))
            r["cut"] = bool(doc.get("analysis", {}).get("cut"))
        except json.JSONDecodeError:
            r["finished"] = False
            r["why"] = "unreadable facts document"
    out.unlink(missing_ok=True)
    err.unlink(missing_ok=True)
    return r


def geomean(xs: list) -> float | None:
    xs = [x for x in xs if x and x > 0]
    return math.exp(sum(math.log(x) for x in xs) / len(xs)) if xs else None


def pooled(runs: dict) -> dict:
    """The scoreboard's pooled indices over runs {module: result}."""
    tiers, facts = Counter(), Counter()
    for r in runs.values():
        if r.get("finished"):
            tiers.update(r.get("tiers", {}))
            facts.update(r.get("facts", {}))
    nt = sum(tiers.values())
    nf = facts["unknown"] + facts["partial"] + facts["exact"]
    walls = [r.get("wall_s") for r in runs.values()]
    return {
        "modules": len(runs),
        "finished": sum(1 for r in runs.values() if r.get("finished")),
        "cut": sum(1 for r in runs.values() if r.get("cut")),
        "verdicts": nt,
        "proven_pct": 100 * tiers["proven"] / nt if nt else None,
        "possibly_pct": 100 * tiers["possibly"] / nt if nt else None,
        "facts": nf,
        "known_facts_pct": 100 * (facts["partial"] + facts["exact"]) / nf if nf else None,
        "exact_facts_pct": 100 * facts["exact"] / nf if nf else None,
        "wall_geomean_s": geomean(walls),
        "wall_total_s": round(sum(w or 0 for w in walls), 1),
        "rss_geomean_mb": geomean([r.get("rss_mb") for r in runs.values() if r.get("finished")]),
        "rss_max_mb": max((r.get("rss_mb") or 0 for r in runs.values()), default=None),
        "skipped_calls": sum(r.get("skipped_calls", 0) for r in runs.values()),
        "work": sum(r.get("work", 0) for r in runs.values()),
    }


def metrics(binary: Path, mode: str, limits: str | None, only: str | None, jobs: int, out_dir: Path, corpus: str = "heldout") -> dict:
    lim = limits
    timeout = DEFAULT_TIMEOUT_S
    if mode == "nodeadline":
        lim = ",".join(x for x in [limits, "ms=100000000"] if x)
        timeout = NODEADLINE_TIMEOUT_S
    if corpus == "tracked":
        # (the tuned-on set, measured the same way: bench/tracked.py's)
        import tracked
        return tracked.metrics(binary, mode, limits, only, jobs, out_dir)
    todo = []
    for p in select(manifest(), only):
        d = fetch_one(p)
        for m in p["modules"]:
            todo.append((p, m, d / m))

    def one(t):
        p, m, path = t
        r = run_module(binary, path, lim, timeout, out_dir)
        r["kind"] = p["kind"]
        print(f"  {p['name']}/{m}: {'ok' if r['finished'] else r.get('why')} {r.get('wall_s')} s"
              f"{' cut' if r.get('cut') else ''} {r.get('rss_mb')} MB", file=sys.stderr, flush=True)
        return f"{p['name']}/{m}" if m else p["name"], r

    with ThreadPoolExecutor(jobs) as ex:
        runs = dict(ex.map(one, todo))
    by_kind = {k: pooled({m: r for m, r in runs.items() if r["kind"] == k}) for k in sorted({r["kind"] for r in runs.values()})}
    return {"corpus": corpus, "mode": mode, "limits": lim, "binary": str(binary), "pooled": pooled(runs), "by_kind": by_kind, "runs": runs}


def fmt(x, d=1) -> str:
    return "–" if x is None else f"{x:.{d}f}"


def print_pooled(name: str, s: dict) -> None:
    print(f"{name:<24} {s['finished']}/{s['modules']} finished, {s['cut']} cut; proven {fmt(s['proven_pct'])}% "
          f"possibly {fmt(s['possibly_pct'])}% of {s['verdicts']}; known facts {fmt(s['known_facts_pct'])}% "
          f"(exact {fmt(s['exact_facts_pct'])}%) of {s['facts']}; wall geomean {fmt(s['wall_geomean_s'], 2)} s "
          f"(total {s['wall_total_s']} s); RSS geomean {fmt(s['rss_geomean_mb'], 0)} MB (max {s['rss_max_mb']}); "
          f"skipped calls {s['skipped_calls']}")


def cmd_metrics(a) -> None:
    res = metrics(Path(a.rowan).resolve(), a.mode, a.limits, a.only, a.j, WORK / "logs" / a.mode, a.corpus)
    print_pooled("all", res["pooled"])
    for k, s in res["by_kind"].items():
        print_pooled(k, s)
    if a.json:
        Path(a.json).write_text(json.dumps(res, indent=1))


def cmd_sweep(a) -> None:
    variants = [("current", Path(a.rowan).resolve(), None)]
    for k, v in (("budget", 4096), ("ms", 10000)):
        variants += [(f"{k}=x0.5", Path(a.rowan).resolve(), f"{k}={v // 2}"), (f"{k}=x2", Path(a.rowan).resolve(), f"{k}={v * 2}")]
    for spec in (a.bins.split(",") if a.bins else []):
        name, b = spec.split("=/", 1)  # (names and paths may hold "=": `unroll=x2=/abs/path`)
        b = "/" + b
        variants.append((name, Path(b), None))
    if a.variants:
        variants = [v for v in variants if v[0] in a.variants.split(",")]
    out = {}
    for name, b, lim in variants:
        print(f"[{name}]", file=sys.stderr, flush=True)
        res = metrics(b, a.mode, lim, a.only, a.j, WORK / "logs" / f"sweep-{name}", a.corpus)
        out[name] = res
        print_pooled(name, res["pooled"])
        if a.json:
            Path(a.json).write_text(json.dumps(out, indent=1))


# ---------------------------------------------------------------- sound: the watched statements
SIMPLE = (ast.Assign, ast.AnnAssign, ast.AugAssign)


def target_names(t: ast.expr) -> list[str]:
    if isinstance(t, ast.Name):
        return [t.id]
    if isinstance(t, (ast.Tuple, ast.List)):
        return [n for e in t.elts for n in target_names(e)]
    if isinstance(t, ast.Starred):
        return target_names(t.value)
    return []


def has_suspend(n: ast.AST) -> bool:
    return any(isinstance(x, (ast.Yield, ast.YieldFrom, ast.Await, ast.NamedExpr)) for x in ast.walk(n))


def watched(src: str) -> list[dict]:
    """The statements to watch: simple assignments binding names, and a function's first statement
    (for its parameters), each alone on its lines; with their names and scope."""
    tree = ast.parse(src)
    out = []

    def body_lists(node):
        for field in ("body", "orelse", "finalbody"):
            b = getattr(node, field, None)
            if isinstance(b, list) and b and isinstance(b[0], ast.stmt):
                yield b
        for h in getattr(node, "handlers", []) or []:
            yield h.body
        for c in getattr(node, "cases", []) or []:
            yield c.body

    def visit(node, scope: str, in_async: bool, func: int = 0):
        for b in body_lists(node):
            lines = Counter()
            for s in b:
                for ln in {s.lineno, s.end_lineno}:
                    lines[ln] += 1
            for i, s in enumerate(b):
                alone = lines[s.lineno] == 1 and lines[s.end_lineno] == 1 and s.lineno != getattr(node, "lineno", 0)
                names = []
                if isinstance(s, ast.Assign):
                    names = [n for t in s.targets for n in target_names(t)]
                elif isinstance(s, ast.AnnAssign) and s.value is not None:
                    names = target_names(s.target)
                elif isinstance(s, ast.AugAssign):
                    names = target_names(s.target)
                if i == 0 and b is getattr(node, "body", None) and isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) \
                        and isinstance(s, SIMPLE + (ast.Expr,)):
                    a = node.args
                    names = names + [x.arg for x in a.posonlyargs + a.args + a.kwonlyargs] + [x.arg for x in (a.vararg, a.kwarg) if x]
                names = list(dict.fromkeys(names))
                # (class bodies: the golden document records no checks there)
                if names and alone and scope != "class" and not in_async and not has_suspend(s) and isinstance(s, SIMPLE + (ast.Expr,)):
                    out.append({"start": s.lineno, "end": s.end_lineno, "names": names, "scope": scope, "func": func})
                if isinstance(s, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    visit(s, "function", isinstance(s, ast.AsyncFunctionDef), s.lineno)
                elif isinstance(s, ast.ClassDef):
                    visit(s, "class" if scope != "function" else "function", False, func)
                else:
                    visit(s, scope, in_async, func)

    visit(tree, "module", False)
    return out


def annotate(src: str, stmts: list[dict]) -> str:
    """The module with `#@general NAME; …` at the end line of each watched statement."""
    lines = src.split("\n")
    for s in stmts:
        i = s["end"] - 1
        lines[i] = lines[i] + "  #@general " + "; ".join(s["names"])
    return "\n".join(lines)


def venv_python(p: dict) -> Path:
    """The package's test venv (Python 3.14 by uv, the manifest's test deps), cached by them."""
    d = WORK / "venv" / p["name"]
    stamp = json.dumps(p["tests"]["deps"])
    py = d / "bin" / "python"
    # (a venv restored from CI's cache whose interpreter is gone is made again)
    if (d / ".stamp").exists() and (d / ".stamp").read_text() == stamp and subprocess.run([str(py), "-c", ""], capture_output=True).returncode == 0:
        return py
    shutil.rmtree(d, ignore_errors=True)
    subprocess.run(["uv", "venv", "-q", "--python", "3.14", str(d)], check=True, stdin=subprocess.DEVNULL)
    subprocess.run(["uv", "pip", "install", "-q", "--python", str(py), *p["tests"]["deps"]], check=True, stdin=subprocess.DEVNULL)
    (d / ".stamp").write_text(stamp)
    return py


def golden_claims(binary: Path, p: dict, mod: str, stmts: list[dict]) -> tuple[dict, list]:
    """rowan's rows and verdicts for the annotated copy of the module (a mirror of the sdist, so
    its imports resolve as in the original): rows by (end line, name), the verdicts' JSON."""
    d = src_dir(p)
    mirror = SCRATCH / "annot" / p["name"]
    if not (mirror / ".stamp").exists() or (mirror / ".stamp").read_text() != p["sha256"]:
        shutil.rmtree(mirror, ignore_errors=True)
        shutil.copytree(d, mirror)
        (mirror / ".stamp").write_text(p["sha256"])
    src = (d / mod).read_text()
    (mirror / mod).write_text(annotate(src, stmts))
    # (a deadline the budget comes before: the claims do not depend on the machine's load)
    env = dict(os.environ, ROWAN_LIMITS=CLAIM_LIMITS)
    so, se = SCRATCH / "annot" / f"{p['name']}.out", SCRATCH / "annot" / f"{p['name']}.err"
    try:
        with open(so, "w") as o, open(se, "w") as e:
            rc, _ = run_guarded([str(binary), str(mirror / mod), "--golden"], CLAIM_TIMEOUT_S, stdout=o, stderr=e, env=env)
        doc = json.loads(so.read_text())
    except (json.JSONDecodeError, OSError):
        rc, doc = 1, {"rows": [], "verdicts": [], "error": se.read_text(errors="replace")[-500:]}
    finally:
        (mirror / mod).write_text(src)
    if "error" not in doc and rc != 0:
        doc["error"] = se.read_text(errors="replace")[-500:]
    so.unlink(missing_ok=True)
    se.unlink(missing_ok=True)
    rows: dict = {}
    for rc in doc["rows"]:
        if rc.get("file", "") == "":
            rows.setdefault((rc["line"], rc["name"]), []).append(rc)
    return rows, [v for v in doc["verdicts"] if (v.get("file") or "") == ""], doc.get("error")


def join_claim(rcs: list[dict]) -> dict:
    v = {"lits": set(), "unknown": False, "objects": set(), "types": set(), "untyped": False, "rows": []}
    for rc in rcs:
        x = rc["value"]
        v["lits"] |= set(x["lits"])
        v["unknown"] |= x["unknown"]
        v["objects"] |= set(x["objects"])
        v["types"] |= set(x.get("types", []))
        v["untyped"] |= x.get("untyped", x["unknown"])
        v["rows"].append(rc["row"])
    return v


def admits(claim: dict, tname: str) -> bool:
    if not claim["unknown"]:
        return False
    return claim["untyped"] or tname in claim["types"] or (tname == "bool" and "int" in claim["types"])


def row_problem(claim: dict, keys: list[str], kind: str) -> str | None:
    labels = claim["objects"]
    dicts = [l for l in labels if l.startswith("dict")]
    if not labels or None in claim["rows"] or (kind == "dict") != (len(dicts) == len(labels)) or (kind != "dict" and dicts):
        return None
    ks = set(keys)
    present, definite, absent, closed = set(), None, None, True
    for r in claim["rows"]:
        present |= set(r["present"])
        definite = set(r["definite"]) if definite is None else definite & set(r["definite"])
        ra = set(r["absent"]) if not r["closed"] else None
        absent = ra if absent is None else (absent & ra if ra is not None else absent)
        closed &= r["closed"]
    # (an instance's attributes rowan knows include its class's: properties, slots, methods)
    if kind == "dict" and definite - ks:
        return f"keys {sorted(ks)} lack definite {sorted(definite - ks)}"
    if absent and absent & ks:
        return f"keys {sorted(ks)} hold absent {sorted(absent & ks)}"
    if closed and ks - present:
        return f"keys {sorted(ks)} not within closed present {sorted(present)}"
    return None


def same_literal(data: str | None, lits: set) -> bool:
    """Is the runtime value's repr one of the literals, by value (`b'a\\'b'` is `b"a'b"`)?"""
    if data is None:
        return False
    if data in lits:
        return True
    try:
        v = ast.literal_eval(data)
    except (ValueError, SyntaxError, MemoryError, RecursionError):
        return False
    for x in lits:
        try:
            w = ast.literal_eval(x)
        except (ValueError, SyntaxError, MemoryError, RecursionError):
            continue
        if type(w) is type(v) and w == v:
            return True
    return False


def value_problem(claim: dict, s: list) -> str | None:
    kind, data, tname = s
    lits = claim["lits"]
    if kind in ("lit", "model") and same_literal(data, lits):
        return None
    if kind == "model":
        # (a datetime / Decimal value rowan holds as a literal: compared by its repr)
        kind = "obj"
    if kind == "lit":
        if admits(claim, tname):
            return None
        if data is None and any(x.startswith(("'", '"', "b'", 'b"')) for x in lits) and tname in ("str", "bytes"):
            return None  # (a long str: its literal not compared)
        return f"{data or tname} not among {sorted(lits)}" + (f" / unknowns of {sorted(claim['types'])}" if claim["unknown"] else "") + (f" / objects {sorted(claim['objects'])}" if claim["objects"] else "")
    if tname == "tuple" and (same_literal(data, lits) or (data is None and any(x.startswith("(") for x in lits))):
        return None  # (a tuple of scalars: one of the literals; a long one's not compared)
    if not claim["objects"] and not claim["unknown"]:
        return f"{tname} {data or ''} where rowan claims only literals {sorted(lits)}"
    if not claim["objects"] and not claim["untyped"] and tname not in claim["types"]:
        return f"{tname} where rowan claims literals {sorted(lits)} or unknowns of {sorted(claim['types'])}"
    if kind in ("dict", "inst") and data is not None:
        return row_problem(claim, data, kind)
    return None


def run_tests(p: dict, points: dict, roots: list[str], out: Path) -> dict:
    py = venv_python(p)
    # (resolved, as check_package's: the tests' file names are the watched points' wherever
    # bench/work/heldout/src is linked from)
    d = src_dir(p).resolve()
    # (files an sdist leaves out that its tests import: the manifest's `shims`)
    for rel, text in p["tests"].get("shims", {}).items():
        (d / rel).write_text(text)
    spec = SCRATCH / "points" / f"{p['name']}.json"
    spec.parent.mkdir(parents=True, exist_ok=True)
    spec.write_text(json.dumps({"files": points, "roots": roots}))
    env = dict(os.environ, HELDOUT_POINTS=str(spec), HELDOUT_OUT=str(out), PYTHONDONTWRITEBYTECODE="1",
               PYTHONPATH=os.pathsep.join([str(HERE / "heldout"), str(d / p["root"])]))
    cmd = [str(py), "-m", "pytest", "-p", "heldout_trace", "-p", "no:cacheprovider", "-q", "--color=no", "--continue-on-collection-errors",
           *p["tests"]["args"], *p["tests"]["paths"]]
    t0 = time.monotonic()
    try:
        r = subprocess.run(cmd, cwd=d, env=env, capture_output=True, text=True, timeout=TEST_TIMEOUT_S, stdin=subprocess.DEVNULL)
        tail = (r.stdout.strip().splitlines() or [""])[-1]
    except subprocess.TimeoutExpired:
        tail = "timeout"
    return {"summary": tail, "s": round(time.monotonic() - t0, 1)}


def check_package(binary: Path, p: dict, verbose: bool) -> dict:
    d = fetch_one(p).resolve()
    roots = [str(d / p["root"])]
    points, claims, per_mod = {}, {}, {}
    for mod in p["modules"]:
        path = str((d / mod).resolve())
        src = (d / mod).read_text()
        stmts = watched(src)
        rows, verdicts, err = golden_claims(binary, p, mod, stmts)
        # access outcomes: unguarded proven / missing reads reached without a call, one tier per line and key
        tiers: dict = {}
        for v in verdicts:
            # (a chain names the entry function's `def` line, then the calls on the way)
            if len(v["chain"]) > 1 or v.get("guarded") or v["ns"] not in ("item", "attr"):
                continue
            tiers.setdefault((v["line"], v["key"]), set()).add(v["tier"])
        acc = {k: next(iter(t)) for k, t in tiers.items() if len(t) == 1 and next(iter(t)) in ("proven", "missing")}
        points[path] = {"stmts": [[s["start"], s["end"], s["names"]] for s in stmts], "verdict_lines": sorted({ln for ln, _ in acc})}
        per_mod[mod] = {"watched": len(stmts), "claims": len(rows), "access_checks": len(acc), "error": err}
        if err:
            # (rowan failed on the module: a crash, counted apart; nothing of it compared)
            continue
        claims[path] = (mod, stmts, rows, acc)
    obs_file = SCRATCH / "obs" / f"{p['name']}.json"
    obs_file.parent.mkdir(parents=True, exist_ok=True)
    obs_file.unlink(missing_ok=True)
    tests = run_tests(p, points, roots, obs_file)
    res = {"tests": tests, "modules": per_mod, "compared": Counter({"rowan failed": sum(1 for m in per_mod.values() if m["error"])}), "mismatches": []}
    if not obs_file.exists():
        res["error"] = "no observations (the test run failed to start)"
        return res
    obs = json.loads(obs_file.read_text())
    cmp = res["compared"]
    for path, (mod, stmts, rows, acc) in claims.items():
        # the functions that had a run for any call (a general row on any of their statements)
        fn_general = {s["func"] for s in stmts if any(rc.get("general") for n in s["names"] for rc in rows.get((s["end"], n), []))}
        for s in stmts:
            o = obs["values"].get(f"{path}:{s['end']}")
            if o is None:
                continue
            cmp["statements reached"] += 1
            for name, summaries in o["names"].items():
                rcs = rows.get((s["end"], name), [])
                if s["scope"] == "function":
                    rcs = [rc for rc in rcs if rc.get("general")]
                else:
                    rcs = [rc for rc in rcs if not rc.get("quiet")]
                if not rcs:
                    cmp["no claim"] += 1
                    if s["scope"] != "function" or s["func"] in fn_general:
                        res["mismatches"].append({"what": "reached", "module": mod, "line": s["end"], "name": name,
                                                  "detail": "the tests complete this statement; rowan has no run reaching it"
                                                  + (" for any call" if s["scope"] == "function" else "")})
                    continue
                claim = join_claim(rcs)
                for sm in summaries:
                    cmp["values"] += 1
                    prob = value_problem(claim, sm)
                    if prob:
                        res["mismatches"].append({"what": "value", "module": mod, "line": s["end"], "name": name, "scope": s["scope"], "detail": prob})
        for (ln, key), tier in acc.items():
            o = obs["lines"].get(f"{path}:{ln}")
            if o is None or not o["runs"]:
                continue
            cmp["accesses"] += 1
            raised = o["raised"].get(key, 0)
            if tier == "proven" and raised:
                res["mismatches"].append({"what": "access", "module": mod, "line": ln, "name": key, "detail": f"proven, raised {raised} of {o['runs']} runs"})
            if tier == "missing" and raised < o["runs"]:
                res["mismatches"].append({"what": "access", "module": mod, "line": ln, "name": key, "detail": f"missing, succeeded {o['runs'] - raised} of {o['runs']} runs"})
    res["compared"] = dict(cmp)
    return res


def cmd_sound(a) -> None:
    global SCRATCH
    binary = Path(a.rowan).resolve()
    if a.scratch:
        SCRATCH = Path(a.scratch).resolve()
    out = {}
    for p in select(manifest(), a.only):
        if not p.get("tests"):
            continue
        print(f"[{p['name']}]", file=sys.stderr, flush=True)
        try:
            r = check_package(binary, p, a.v)
        except subprocess.CalledProcessError as e:
            r = {"error": f"setup failed: {e}"}
        out[p["name"]] = r
        c = r.get("compared", {})
        print(f"{p['name']:<18} tests: {r.get('tests', {}).get('summary', r.get('error'))[:60]:<60} reached {c.get('statements reached', 0)}, "
              f"values {c.get('values', 0)}, accesses {c.get('accesses', 0)}, no claim {c.get('no claim', 0)}; mismatches {len(r.get('mismatches', []))}")
        if a.v:
            for m in r.get("mismatches", []):
                print(f"    {m['what']:<7} {m['module']}:{m['line']} {m['name']}: {m['detail'][:200]}")
        if a.json:
            Path(a.json).write_text(json.dumps(out, indent=1, default=list))
    tot = Counter()
    for r in out.values():
        tot.update(r.get("compared", {}))
        tot.update(Counter(m["what"] for m in r.get("mismatches", [])))
    print(f"total: {dict(tot)}")
    if a.baseline or a.write_baseline:
        sys.exit(check_baseline(out, a))


# ---------------------------------------------------------------- sound: the known candidates
def candidate_sites(out: dict) -> dict:
    """Each candidate site (`PKG WHAT MODULE:LINE NAME`; a module rowan failed on `PKG crashed
    MODULE`, a package whose tests did not run `PKG error`) with its first detail."""
    sites: dict = {}
    for pkg, r in out.items():
        if r.get("error"):
            sites[f"{pkg} error"] = r["error"]
        for mod, info in r.get("modules", {}).items():
            if info.get("error"):
                sites[f"{pkg} crashed {mod}"] = info["error"][-200:]
        for m in r.get("mismatches", []):
            sites.setdefault(f"{pkg} {m['what']} {m['module']}:{m['line']} {m['name']}", m["detail"][:200])
    return sites


def check_baseline(out: dict, a) -> int:
    """Compare the candidate sites with the checked-in baseline (BASELINE: identifiers of known
    candidates, each with the kind that explains it; no work items, AGENTS.md): new ones fail
    (exit 1), gone ones are reported. `--report` writes the comparison as Markdown;
    `--write-baseline` writes today's sites, keeping known kinds (new ones `unexplained`)."""
    base = json.loads(Path(a.baseline or BASELINE).read_text()) if Path(a.baseline or BASELINE).exists() else {"kinds": {}, "sites": {}}
    sites = candidate_sites(out)
    known = base["sites"]
    new = sorted(k for k in sites if k not in known)
    gone = sorted(k for k in known if k not in sites)
    by_kind = Counter(known[k] for k in sites if k in known)
    lines = [f"### held-out soundness (bench/heldout.py sound): {len(sites)} candidate sites, {len(new)} new, {len(gone)} gone", ""]
    lines += ["| kind | sites | what explains them |", "| --- | --- | --- |"]
    lines += [f"| {k} | {by_kind[k]} | {base['kinds'].get(k, '')} |" for k in base["kinds"]]
    if new:
        lines += ["", "New candidate sites (an unsound claim to shrink into a repro, or a limit of the matching to add to the baseline with its kind):", ""]
        lines += [f"- `{k}`: {sites[k]}" for k in new]
    if gone:
        lines += ["", "No longer found (remove from the baseline):", ""] + [f"- `{k}`" for k in gone]
    text = "\n".join(lines) + "\n"
    print(text)
    if a.report:
        Path(a.report).write_text(text)
    if a.write_baseline:
        doc = {k: v for k, v in base.items() if k != "sites"}
        doc["sites"] = {k: known.get(k, "unexplained") for k in sorted(sites)}
        Path(a.write_baseline).write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n")
    return 1 if new else 0


# ---------------------------------------------------------------- rotate
TOP_PYPI = "https://hugovk.github.io/top-pypi-packages/top-pypi-packages-30-days.min.json"
COMPILED = (".c", ".cc", ".cpp", ".cxx", ".pyx", ".pxd", ".rs", ".f90", ".go")
SKIP_PARTS = {"tests", "test", "testing", "docs", "doc", "examples", "example", "benchmarks", "bench", "scripts", "tools", "build"}


def norm(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def taken_names() -> set[str]:
    """Distribution and import names of everything tracked or tested (normalised): the eight's
    distributions, the cost set's packages and module names, the site-packages scenarios' pins,
    every top-level import of the golden suites, the current held-out corpus."""
    import prepare
    import sitepackages

    names = {n for n in ("rich", "click", "pip", "timm", "sktime")}
    for v in vars(prepare).values():
        if isinstance(v, list) and v and all(isinstance(x, tuple) and len(x) == 2 for x in v):
            names |= {n for n, _ in v}
    for spec in sitepackages.SCENARIOS.values():
        names |= {re.split(r"[=<>\[]", x)[0] for x in spec.get("pins", [])}
    cs = json.loads((HERE / "costset" / "manifest.json").read_text())
    names |= {p["name"] for p in cs["packages"]} | {m["name"].split("_")[0] for m in cs["modules"]}
    names |= {p["name"] for p in manifest()}
    imp = re.compile(r"^\s*(?:import|from)\s+([A-Za-z_]\w*)", re.M)
    for f in (ROOT / "tests").rglob("*.py"):
        names |= set(imp.findall(f.read_text(errors="replace")))
    return {norm(n) for n in names}


def pypi_json(name: str) -> dict | None:
    try:
        return json.loads(urllib.request.urlopen(f"https://pypi.org/pypi/{name}/json", timeout=30).read())
    except OSError:
        return None


def latest_sdist(meta: dict, snapshot: str) -> tuple[str, dict, list] | None:
    """The newest release uploaded before the snapshot with an sdist (no pre-releases or yanked
    files): (version, the sdist's file entry, all its files)."""
    best = None
    for ver, files in meta.get("releases", {}).items():
        if not files or re.search(r"[a-zA-Z]", ver.replace("post", "")):
            continue
        sd = [f for f in files if f["packagetype"] == "sdist" and not f.get("yanked") and f["upload_time_iso_8601"][:10] < snapshot]
        if sd and (best is None or sd[0]["upload_time_iso_8601"] > best[1]["upload_time_iso_8601"]):
            best = (ver, sd[0], files)
    return best


def draw_one(name: str, snapshot: str, taken: set[str]) -> tuple[dict | None, str]:
    """A manifest entry for the candidate, or None with the reason it fails."""
    meta = pypi_json(name)
    if not meta:
        return None, "not on PyPI"
    found = latest_sdist(meta, snapshot)
    if not found:
        return None, "no sdist before the snapshot"
    ver, sd, files = found
    if any(f["packagetype"] == "bdist_wheel" and not f["filename"].endswith("-none-any.whl") for f in files):
        return None, "platform wheels (not pure Python)"
    data = urllib.request.urlopen(sd["url"], timeout=120).read()
    if hashlib.sha256(data).hexdigest() != sd["digests"]["sha256"]:
        return None, "sha256 mismatch"
    if sd["filename"].endswith(".zip"):
        z = zipfile.ZipFile(io.BytesIO(data))
        members = {n: z.read(n) for n in z.namelist() if n.endswith((".py",) + COMPILED)}
    else:
        with tarfile.open(fileobj=io.BytesIO(data)) as t:
            members = {m.name: t.extractfile(m).read() for m in t.getmembers() if m.isfile() and m.name.endswith((".py",) + COMPILED)}
    rel = {n.split("/", 1)[1]: b for n, b in members.items() if "/" in n}
    if any(n.endswith(COMPILED) for n in rel):
        return None, "compiled sources in the sdist"
    code = {n: b for n, b in rel.items() if n.endswith(".py") and not SKIP_PARTS & set(n.split("/")[:-1])
            and not n.split("/")[-1].startswith(("test_", "conftest", "setup.py")) and n.split("/")[0] not in ("setup.py",)}
    tops = {norm(n.split("/")[1] if n.startswith("src/") else n.split("/")[0].removesuffix(".py")) for n in code}
    if norm(name) in taken or tops & taken:
        return None, f"overlaps a tracked set or a golden case ({sorted((tops | {norm(name)}) & taken)})"
    mods = sorted(code, key=lambda n: -code[n].count(b"\n"))[:2]
    lines = sum(code[m].count(b"\n") for m in mods)
    if not mods or not 200 <= lines <= 8000:
        return None, f"its two largest modules have {lines} lines"
    root = "src" if mods[0].startswith("src/") else "."
    tdirs = sorted({n.split("/")[0] for n in rel if n.split("/")[0] in ("tests", "test")})
    reqs = [r.split(";")[0].split("[")[0].strip() for r in (meta["info"].get("requires_dist") or []) if "extra ==" not in r]
    return {"name": meta["info"]["name"], "version": ver, "kind": "unclassified", "why": "",
            "sdist": sd["filename"], "url": sd["url"], "sha256": sd["digests"]["sha256"], "root": root,
            "modules": mods, "tests": {"paths": tdirs, "deps": ["pytest", *[re.split(r"[<>=!~ (]", r)[0] for r in reqs if r]], "args": []} if tdirs else None}, f"{lines} lines"


def cmd_rotate(a) -> None:
    if a.apply:
        return rotate_apply(Path(a.apply))
    if not a.draw:
        sys.exit("rotate: --draw or --apply PROPOSAL.json")
    src = a.candidates or TOP_PYPI
    if src.startswith("http"):
        doc = json.loads(urllib.request.urlopen(src, timeout=60).read())
        cands = [r["project"] for r in doc["rows"]]
    else:
        cands = [x.strip() for x in Path(src).read_text().splitlines() if x.strip() and not x.startswith("#")]
    random.Random(a.seed).shuffle(cands)
    taken, out = taken_names(), []
    for name in cands:
        if len(out) >= a.n:
            break
        try:
            e, why = draw_one(name, a.snapshot, taken)
        except (OSError, tarfile.TarError, zipfile.BadZipFile, KeyError) as ex:
            e, why = None, f"failed: {ex}"
        print(f"  {name}: {'taken, ' if e else ''}{why}", file=sys.stderr, flush=True)
        if e:
            out.append(e)
            taken.add(norm(e["name"]))
    doc = {"snapshot": a.snapshot, "drawn": {"seed": a.seed, "candidates": src}, "packages": out}
    Path(a.out).write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n")
    print(f"{len(out)} packages proposed in {a.out}: review kinds, whys and test arguments, then `rotate --apply {a.out}`")


def rotate_apply(proposal: Path) -> None:
    """The current corpus into the cost set's manifest, the proposal as the corpus, the baseline reset."""
    new = json.loads(proposal.read_text())
    cs_file = HERE / "costset" / "manifest.json"
    cs = json.loads(cs_file.read_text())
    old = json.loads(MANIFEST.read_text())
    have = {p["name"] for p in cs["packages"]}
    for p in old["packages"]:
        d = fetch_one(p)
        if p["name"] not in have:
            cs["packages"].append({k: p[k] for k in ("name", "version", "url", "sha256")})
        for m in p["modules"]:
            cs["modules"].append({"name": f"{norm(p['name']).replace('-', '_')}_{Path(m).stem.strip('_')}", "source": "pypi", "kind": p["kind"],
                                  "package": p["name"], "path": m, "sha256": hashlib.sha256((d / m).read_bytes()).hexdigest()})
    cs.setdefault("retired", []).append({"snapshot": old.get("snapshot"), "packages": len(old["packages"]),
                                         "note": "a held-out corpus retired into the tuned-on set by `heldout.py rotate --apply`"})
    cs_file.write_text(json.dumps(cs, indent=1, ensure_ascii=False) + "\n")
    MANIFEST.write_text(json.dumps(new, indent=1, ensure_ascii=False) + "\n")
    base = json.loads(BASELINE.read_text())
    base["sites"] = {}
    base["_doc"] = base.get("_doc", "").split(" Classified in")[0] + f" Reset for the corpus of {new.get('snapshot')}: run `sound --write-baseline` and classify."
    BASELINE.write_text(json.dumps(base, indent=1, ensure_ascii=False) + "\n")
    print(f"retired {len(old['packages'])} packages into {cs_file.relative_to(ROOT)}; {len(new['packages'])} held out; "
          f"next: `heldout.py fetch`, `heldout.py sound --write-baseline {BASELINE.relative_to(ROOT)}`, classify its sites, commit")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    rot = sub.add_parser("rotate")
    rot.add_argument("--draw", action="store_true", help="propose a fresh corpus (changes nothing)")
    rot.add_argument("--apply", help="install a reviewed proposal: the current corpus joins the cost set")
    rot.add_argument("--candidates", help=f"names, one per line, or a top-packages JSON URL (default {TOP_PYPI})")
    rot.add_argument("--snapshot", default=time.strftime("%Y-%m-%d"), help="the latest sdist uploaded before this date")
    rot.add_argument("-n", type=int, default=31)
    rot.add_argument("--seed", type=int, default=0)
    rot.add_argument("--out", default=str(WORK / "proposal.json"))
    for name in ("fetch", "metrics", "sound", "sweep"):
        q = sub.add_parser(name)
        q.add_argument("--only")
        q.add_argument("--rowan", default=str(RUST_BIN))
        q.add_argument("--json")
        q.add_argument("-j", type=int, default=1)
        q.add_argument("-v", action="store_true")
        if name in ("metrics", "sweep"):
            q.add_argument("--mode", default="default", choices=["default", "nodeadline"])
            q.add_argument("--corpus", default="heldout", choices=["heldout", "tracked"])
        if name == "metrics":
            q.add_argument("--limits")
        if name == "sound":
            q.add_argument("--baseline", help=f"compare with the known candidates (default {BASELINE.relative_to(ROOT)}); exit 1 on new ones")
            q.add_argument("--report", help="the comparison as Markdown")
            q.add_argument("--write-baseline", help="write today's candidate sites as a baseline")
            q.add_argument("--scratch", help="annotated copies, observations, test points here (default bench/work/heldout)")
        if name == "sweep":
            q.add_argument("--bins")
            q.add_argument("--variants", help="only these (current, budget=x0.5, …, the --bins names)")
    a = ap.parse_args()
    {"fetch": cmd_fetch, "metrics": cmd_metrics, "sound": cmd_sound, "sweep": cmd_sweep, "rotate": cmd_rotate}[a.cmd](a)


if __name__ == "__main__":
    main()
