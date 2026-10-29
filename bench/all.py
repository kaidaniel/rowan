"""Everything an iteration reports, for main and a branch, in one table (SUBAGENTS.md's hand-back).

    uv run --no-project python bench/all.py [--base REF|BIN|none] [--head BIN] [--only STEP,…]
            [--skip STEP,…] [--seeds A,B] [-j 3] [--out DIR] [--fresh] [--tags vX,…]

`--base` (default `main`): a git ref, built in a scratch worktree under bench/work/all/ (the
binary cached by commit), or a binary; `none` measures the head alone. `--head` (default: this
checkout's engine, built here with `cargo build --release`). Every step runs this checkout's
tests and harnesses on both binaries (newer tests find older bugs, as the scoreboard's).

Steps (STEPS; `--only` / `--skip` by name, a skipped step is listed as such in the table):
  checks, both binaries, up to -j at once (cached by binary, step and this checkout's tests):
    golden       tests/run_golden.py: passed / total
    budget0      the same with ROWAN_LIMITS=budget=0
    snapshots    tests/run_snapshots.py (compares only; never --update)
    fuzz         tests/fuzz.py -n 100 at two fresh seeds (`--seeds`; the same for both): unsound
    protocols    every enumerator step of ci.yml at its bound (bench/protocols.py ci)
    oracle       tests/oracle.py with pandas, pydantic, FastAPI, numpy
    metamorphic  tests/metamorphic.py on every case
    ondemand     tests/ondemand_check.py
    node         vscode-rowan/test.js (real hovers)
    heldsound    bench/heldout.py sound --baseline (~25 min; each binary in a scratch dir of its own)
  once, the head's source:
    cargo        cargo test --release --lib
    clippy       cargo clippy -D clippy::correctness (ci.yml's)
    torch        tests/torch_oracle.py (expectations against real PyTorch; no rowan run)
  source, base ref and head: engine and model lines, `#@todo`s, Analyzer's fields, engine
    functions over 100 lines, the largest engine file
  timing, one run at a time with nothing else running, the two binaries interleaved per program:
    tracked      bench/tracked.py, both modes (31 programs)
    heldout      bench/heldout.py metrics, both modes (52 modules)
    sitepackages bench/sitepackages.py
    hovers       bench/hovers.py and bench/first_hover.py on all eight

Prints one table (metric, base, head) and the tracked set's per-program table; writes them and
every log under bench/work/all/<time>/ (`--out`). From a scratch worktree: `.guard/run` and the
prepared programs, corpora and venvs of the main checkout's bench/work are linked in (gitignored;
nothing tracked is written in the main checkout). `--tags`: then a scoreboard row
(bench/scoreboard.py run --tags), only from a scratch worktree.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import re
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
WORK = HERE / "work" / "all"
CHECKS = ["golden", "budget0", "snapshots", "fuzz", "protocols", "oracle", "metamorphic", "ondemand", "node", "heldsound"]
ONCE = ["cargo", "clippy", "torch"]
TIMING = ["tracked", "heldout", "sitepackages", "hovers"]
STEPS = CHECKS + ONCE + ["source"] + TIMING
ORACLE_DEPS = ["pandas", "pydantic", "pydantic-settings", "fastapi", "httpx", "numpy"]
EIGHT = ["stdlib12k", "email", "rich", "click", "pip", "timm", "sktime", "functional"]
# bench/work entries shared with the main checkout (prepared programs, corpora, venvs)
SHARED = [*EIGHT, "sitepackages", "costset/src", "costset/stdlib", "heldout/src", "heldout/venv"]


def git(*args: str, cwd: Path = ROOT) -> str:
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def main_checkout() -> Path:
    return Path(git("rev-parse", "--path-format=absolute", "--git-common-dir")).parent


# ---------------------------------------------------------------- setup
def setup_worktree() -> Path | None:
    """The guard and shared data, linked in from the main checkout when this is a worktree.
    Returns the guard (None where there is none: CI, other machines)."""
    main = main_checkout()
    guard = ROOT / ".guard" / "run"
    if not guard.exists() and main != ROOT and (main / ".guard" / "run").exists():
        (ROOT / ".guard").symlink_to(main / ".guard")
        print(f"linked {ROOT / '.guard'} -> {main / '.guard'} (remove it before removing the worktree)", file=sys.stderr)
    if main != ROOT:
        for rel in SHARED:
            src, dst = main / "bench" / "work" / rel, HERE / "work" / rel
            if src.exists() and not dst.exists() and not dst.is_symlink():
                dst.parent.mkdir(parents=True, exist_ok=True)
                dst.symlink_to(src)
    return guard if guard.exists() else None


GUARD: Path | None = None


def guarded(cmd: list[str]) -> list[str]:
    """For a harness that runs rowan itself, unguarded (golden, fuzz, the oracle, …)."""
    return [str(GUARD), "-t", "0", *cmd] if GUARD else cmd


def py(*args: str, deps: list[str] = ()) -> list[str]:
    return ["uv", "run", "--no-project", *[x for d in deps for x in ("--with", d)], "python", *args]


def run(cmd: list[str], log: Path, env: dict | None = None, cwd: Path = ROOT) -> tuple[int, str, float]:
    log.parent.mkdir(parents=True, exist_ok=True)
    t0 = time.monotonic()
    with open(log, "w") as f:
        p = subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT, env={**os.environ, **(env or {})}, cwd=cwd, stdin=subprocess.DEVNULL)
    return p.returncode, log.read_text(errors="replace"), round(time.monotonic() - t0, 1)


def build_ref(ref: str) -> Path:
    """The release binary of a commit, built once in a scratch worktree (cached by commit)."""
    sha = git("rev-parse", ref)
    out = WORK / "bin" / sha / "rowan"
    if out.exists():
        return out
    wt = WORK / f"wt-{sha[:10]}"
    if not wt.exists():
        git("worktree", "add", "--detach", str(wt), sha)
    try:
        print(f"building {ref} ({sha[:10]})", file=sys.stderr, flush=True)
        subprocess.run(["cargo", "build", "--release", "--locked"], cwd=wt / "rowan", check=True,
                       env={**os.environ, "CARGO_TARGET_DIR": str(WORK / "target")}, stdout=subprocess.DEVNULL)
        out.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(WORK / "target" / "release" / "rowan", out)
    finally:
        git("worktree", "remove", "--force", str(wt))
    return out


def build_head() -> Path:
    print("building the head (cargo build --release)", file=sys.stderr, flush=True)
    subprocess.run(["cargo", "build", "--release", "--locked"], cwd=ROOT / "rowan", check=True, stdout=subprocess.DEVNULL)
    return ROOT / "rowan" / "target" / "release" / "rowan"


def sha_file(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def tests_fingerprint() -> str:
    """What the checks run (committed and not): the tests, the extension's, ci.yml's steps and the
    two bench harnesses checks use; a cached check is valid for them."""
    paths = ["tests", "vscode-rowan", ".github/workflows/ci.yml", "bench/protocols.py", "bench/heldout.py", "bench/heldout"]
    h = hashlib.sha256(git("ls-files", "-s", *paths).encode())
    h.update(git("diff", "HEAD", "--", *paths).encode())
    return h.hexdigest()[:16]


# ---------------------------------------------------------------- the checks (one binary each)
def num(pat: str, text: str, default=None, last=True):
    ms = re.findall(pat, text, re.M)
    return (ms[-1] if last else ms[0]) if ms else default


def check(step: str, binary: Path, side: str, out: Path, seeds: list[int]) -> dict:
    env = {"ROWAN_BIN": str(binary)}
    log = out / "logs" / f"{step}-{side}.log"
    if step in ("golden", "budget0"):
        rc, text, s = run(guarded(py("tests/run_golden.py")), log, {**env, **({"ROWAN_LIMITS": "budget=0"} if step == "budget0" else {})})
        m = re.findall(r"^\s*(\d+)/(\d+) passed(?:\s+\((\d+) #@todo)?", text, re.M)
        if not m:
            return {"error": text[-300:], "s": s}
        return {"passed": int(m[-1][0]), "total": int(m[-1][1]), "todo": int(m[-1][2] or 0), "s": s}
    if step == "snapshots":
        rc, text, s = run(guarded(py("tests/run_snapshots.py")), log, env)
        return {"ok": rc == 0, "diff": len(re.findall(r"^\s*DIFF ", text, re.M)), "s": s}
    if step == "fuzz":
        res = {"unsound": 0, "checks": 0, "agree": 0, "seeds": seeds, "s": 0.0}
        for sd in seeds:
            rc, text, s = run(guarded(py("tests/fuzz.py", "-n", "100", "--seed", str(sd), "-j", "3", "--out", str(out / f"fuzz-{side}-{sd}"))),
                              out / "logs" / f"fuzz-{side}-{sd}.log", env)
            res["unsound"] += sum(int(x) for x in re.findall(r"checks, (\d+) unsound", text))
            a = re.findall(r"^(\d+)/(\d+) checks agree", text, re.M)
            if a:
                res["agree"] += int(a[-1][0])
                res["checks"] += int(a[-1][1])
            else:
                res["error"] = text[-300:]
            res["s"] += s
        return res
    if step == "protocols":
        js = out / f"protocols-{side}.json"
        rc, text, s = run(py("bench/protocols.py", "ci", "-j", "3", "--rowan", str(binary), "--json", str(js), "--logs", str(out / "logs" / f"protocols-{side}")), log)
        if not js.exists():
            return {"error": text[-300:], "s": s}
        by = json.loads(js.read_text())["by_name"]
        return {"steps": len(by), "points": sum(t["points"] for t in by.values()), "missed": sum(t["missed"] for t in by.values()),
                "failed": sorted(n for n, t in by.items() if t["failed"]), "s": s}
    if step == "oracle":
        rc, text, s = run(guarded(py("tests/oracle.py", deps=ORACLE_DEPS)), log, env)
        m = num(r"^oracle: (\d+)/(\d+) cases agree", text)
        return {"agree": int(m[0]), "total": int(m[1]), "s": s} if m else {"error": text[-300:], "s": s}
    if step == "metamorphic":
        js = out / f"metamorphic-{side}.json"
        rc, text, s = run(py("tests/metamorphic.py", "-j", "3", "--json", str(js)), log, env)
        if not js.exists():
            return {"error": text[-300:], "s": s}
        d = json.loads(js.read_text())
        return {"cases": d["cases"], "places": sum(d["checked"].values()), "missed": sum(d["missed"].values()), "s": s}
    if step == "ondemand":
        rc, text, s = run(guarded(py("tests/ondemand_check.py", "-j", "3")), log, env)
        m = re.search(r"^(\{.*\}) (\d+) contradicting missing", text, re.M)
        return {**json.loads(m.group(1)), "contradicting": int(m.group(2)), "s": s} if m else {"error": text[-300:], "s": s}
    if step == "node":
        rc, text, s = run(guarded(["node", "test.js"]), log, env, cwd=ROOT / "vscode-rowan")
        return {"ok": rc == 0, "s": s}
    if step == "heldsound":
        js = out / f"heldsound-{side}.json"
        rc, text, s = run(py("bench/heldout.py", "sound", "--rowan", str(binary), "--json", str(js), "--baseline", str(HERE / "heldout" / "sound_baseline.json"),
                             "--scratch", str(out / f"heldsound-{side}"), "--report", str(out / f"heldsound-{side}.md")), log)
        m = re.search(r"(\d+) candidate sites, (\d+) new, (\d+) gone", text)
        return {"sites": int(m.group(1)), "new": int(m.group(2)), "gone": int(m.group(3)), "s": s} if m else {"error": text[-300:], "s": s}
    raise ValueError(step)


def once(step: str, out: Path) -> dict:
    log = out / "logs" / f"{step}.log"
    if step == "cargo":
        rc, text, s = run(guarded(["cargo", "test", "--release", "--lib", "--locked"]), log, cwd=ROOT / "rowan")
        m = num(r"test result: \w+\. (\d+) passed; (\d+) failed", text)
        return {"ok": rc == 0, "passed": int(m[0]) if m else None, "failed": int(m[1]) if m else None, "s": s}
    if step == "clippy":
        rc, text, s = run(["cargo", "clippy", "--release", "--locked", "--all-targets", "--", "-D", "clippy::correctness"], log, cwd=ROOT / "rowan")
        return {"ok": rc == 0, "s": s}
    if step == "torch":
        rc, text, s = run(py("tests/torch_oracle.py", deps=["torch", "numpy"]), log, {"UV_EXTRA_INDEX_URL": "https://download.pytorch.org/whl/cpu"})
        m = num(r"^oracle: (\d+)/(\d+) cases agree", text)
        return {"agree": int(m[0]), "total": int(m[1]), "s": s} if m else {"error": text[-300:], "s": s}
    raise ValueError(step)


# ---------------------------------------------------------------- source
def source_files(ref: str | None) -> dict[str, str]:
    """rowan/src/**/*.rs and tests' Python of a ref (None: this checkout's files on disk)."""
    if ref is None:
        out = {str(p.relative_to(ROOT)): p.read_text(errors="replace") for p in (ROOT / "rowan" / "src").rglob("*.rs")}
        out.update({str(p.relative_to(ROOT)): p.read_text(errors="replace") for p in (ROOT / "tests").rglob("*.py")})
        return out
    names = [n for n in git("ls-tree", "-r", "--name-only", ref, "rowan/src", "tests").splitlines() if n.endswith((".rs", ".py"))]
    data = subprocess.run(["git", "cat-file", "--batch"], cwd=ROOT, input="".join(f"{ref}:{n}\n" for n in names).encode(), capture_output=True, check=True).stdout
    out, at = {}, 0
    for n in names:
        nl = data.index(b"\n", at)
        size = int(data[at:nl].split()[2])
        out[n] = data[nl + 1:nl + 1 + size].decode(errors="replace")
        at = nl + 1 + size + 1
    return out


def source(ref: str | None) -> dict:
    files = source_files(ref)
    engine = {n: t for n, t in files.items() if n.startswith("rowan/src/") and n.endswith(".rs") and "/models/" not in n}
    models = {n: t for n, t in files.items() if n.startswith("rowan/src/models/")}
    long_fns = 0
    for t in engine.values():
        lines = t.split("\n")
        for i, ln in enumerate(lines):
            m = re.match(r"^(\s*)(?:pub(?:\([^)]*\))? )?(?:const )?(?:async )?(?:unsafe )?fn \w+", ln)
            if not m or ln.rstrip().endswith(";"):
                continue
            close = m.group(1) + "}"
            for j in range(i + 1, len(lines)):
                if lines[j].rstrip() == close:
                    long_fns += j - i + 1 > 100
                    break
    largest = max(engine.items(), key=lambda kv: kv[1].count("\n"))
    mod = files.get("rowan/src/interp/mod.rs", "")
    body = re.search(r"pub struct Analyzer\b[^{]*\{(.*?)\n\}", mod, re.S)
    fields = len(re.findall(r"^\s*(?:pub(?:\([^)]*\))? )?\w+\s*:", body.group(1), re.M)) if body else None
    todos = sum(t.count("#@todo") for n, t in files.items() if n.startswith("tests/"))
    # (lines as the scoreboard's `size` counts them: newlines + 1 per file)
    return {"engine_lines": sum(t.count("\n") + 1 for t in engine.values()), "model_lines": sum(t.count("\n") + 1 for t in models.values()),
            "todos": todos, "analyzer_fields": fields, "fns_over_100": long_fns,
            "largest": f"{largest[0].removeprefix('rowan/src/')} ({largest[1].count(chr(10)):,})"}


# ---------------------------------------------------------------- timing (interleaved)
def timing(step: str, bins: dict[str, Path], out: Path) -> dict:
    """{side: result}; the two binaries alternate per program (ABBA), nothing else running."""
    sides = list(bins)
    res: dict = {s: {} for s in sides}
    order = lambda i: sides if i % 2 == 0 else sides[::-1]  # noqa: E731
    t0 = time.monotonic()
    if step in ("tracked", "heldout"):
        import heldout
        import tracked
        if step == "tracked":
            todo = [(p["name"], p, tracked.path_of(p)) for p in tracked.programs()]
        else:
            todo = [(f"{p['name']}/{m}", p, heldout.fetch_one(p) / m) for p in heldout.manifest() for m in p["modules"]]
        for i, (name, p, path) in enumerate(todo):
            for mode in tracked.MODES:
                for s in order(i):
                    if step == "tracked":
                        r = tracked.run_one(bins[s], p, path, mode, None, out / "logs" / f"tracked-{s}")
                        r["lines"] = tracked.lines_of(p, path)
                    else:
                        r = heldout.run_module(bins[s], path, tracked.limits_for(mode, None),
                                               heldout.DEFAULT_TIMEOUT_S if mode == "default" else heldout.NODEADLINE_TIMEOUT_S, out / "logs" / f"heldout-{s}" / mode)
                        r["kind"] = p["kind"]
                    res[s].setdefault(mode, {})[name] = r
                    print(f"  {step} {name} {mode} {s}: {r.get('wall_s')} s {r.get('rss_mb')} MB{'' if r.get('finished') else ' ' + str(r.get('why'))}", file=sys.stderr, flush=True)
        for s in sides:
            (out / f"{step}-{s}.json").write_text(json.dumps({"set": step, "binary": str(bins[s]), "modes": {m: {"runs": rs} for m, rs in res[s].items()}}, indent=1))
    elif step == "sitepackages":
        for s in sides:
            js = out / f"sitepackages-{s}.json"
            run(py("bench/sitepackages.py", "--rowan", str(bins[s]), "--json", "--no-check", "--out", str(js)), out / "logs" / f"sitepackages-{s}.log")
            res[s] = json.loads(js.read_text()) if js.exists() else {"error": "no result"}
    elif step == "hovers":
        for i, p in enumerate(EIGHT):
            for s in order(i):
                d = out / f"hovers-{s}"
                run(py("bench/hovers.py", str(bins[s]), p, "-j", "3", "--out", str(d)), out / "logs" / f"hovers-{s}-{p}.log")
                run(py("bench/first_hover.py", str(bins[s]), p, "--out", str(d)), out / "logs" / f"first-{s}-{p}.log")
                h = d / f"{p}-hovers.json"
                f = d / f"{p}-first.json"
                res[s][p] = {"hovers": json.loads(h.read_text())["summary"] if h.exists() else None,
                             "first": json.loads(f.read_text())["summary"] if f.exists() else None}
                print(f"  hovers {p} {s}: {(res[s][p]['hovers'] or {}).get('p50')} ms p50", file=sys.stderr, flush=True)
    res["s"] = round(time.monotonic() - t0, 1)
    return res


# ---------------------------------------------------------------- the table
def fmt(x, d=1) -> str:
    if x is None:
        return "–"
    if isinstance(x, float):
        return f"{x:,.{d}f}"
    if isinstance(x, int):
        return f"{x:,}"
    return str(x)


def rows(r: dict, sides: list[str]) -> list[tuple]:
    """(group, metric, {side: text}) for the table."""
    import heldout

    out = []

    def add(group, metric, f, d=1):
        vals = {}
        for s in sides:
            try:
                vals[s] = fmt(f(s), d)
            except (KeyError, TypeError, ValueError, ZeroDivisionError, AttributeError):
                vals[s] = "–"
        out.append((group, metric, vals))

    c = r.get("checks", {})

    def ck(step):
        return lambda s: c[step][s]

    def skipped(step, group, metric):
        out.append((group, metric, {s: "not run" for s in sides}))

    if "golden" in c:
        add("soundness", "golden passed", lambda s: f"{ck('golden')(s)['passed']}/{ck('golden')(s)['total']}")
    else:
        skipped("golden", "soundness", "golden passed")
    for step, metric, f in [
        ("budget0", "budget=0 golden passed", lambda s: f"{ck('budget0')(s)['passed']}/{ck('budget0')(s)['total']}"),
        ("snapshots", "snapshots", lambda s: "same" if ck("snapshots")(s)["ok"] else f"{ck('snapshots')(s)['diff']} differ"),
        ("fuzz", "fuzz unsound (seeds " + ",".join(map(str, r.get("seeds", []))) + ")", lambda s: f"{ck('fuzz')(s)['unsound']} of {ck('fuzz')(s)['checks']:,} checks"),
        ("protocols", "ci.yml enumerators: missed points", lambda s: f"{ck('protocols')(s)['missed']} of {ck('protocols')(s)['points']:,} ({len(ck('protocols')(s)['failed'])} failed)"),
        ("oracle", "runtime oracle", lambda s: f"{ck('oracle')(s)['agree']}/{ck('oracle')(s)['total']}"),
        ("metamorphic", "metamorphic: missed places", lambda s: f"{ck('metamorphic')(s)['missed']} of {ck('metamorphic')(s)['places']:,}"),
        ("ondemand", "on demand: contradicting missing", lambda s: ck("ondemand")(s)["contradicting"]),
        ("node", "node test.js", lambda s: "pass" if ck("node")(s)["ok"] else "FAIL"),
        ("heldsound", "held-out sound: candidate sites (new)", lambda s: f"{ck('heldsound')(s)['sites']} ({ck('heldsound')(s)['new']} new)"),
    ]:
        if step in c:
            add("soundness", metric, f)
        else:
            skipped(step, "soundness", metric)
    o = r.get("once", {})
    for step, metric, f in [("cargo", "cargo test (head)", lambda: f"{'pass' if o['cargo']['ok'] else 'FAIL'} ({o['cargo']['passed']} tests)"),
                            ("clippy", "clippy correctness (head)", lambda: "pass" if o["clippy"]["ok"] else "FAIL"),
                            ("torch", "torch oracle (head)", lambda: f"{o['torch']['agree']}/{o['torch']['total']}")]:
        if step in o:
            v = f() if "error" not in o[step] else "error"
            out.append(("soundness", metric, {s: (v if s == sides[-1] else "") for s in sides}))
        else:
            skipped(step, "soundness", metric)
    src = r.get("source", {})
    if src:
        for k, label in [("engine_lines", "engine lines"), ("model_lines", "model lines"), ("todos", "#@todo (tests)"), ("analyzer_fields", "Analyzer fields"),
                         ("fns_over_100", "engine fns over 100 lines"), ("largest", "largest engine file")]:
            add("code", label, lambda s, k=k: src[s][k])
    t = r.get("timing", {})
    if "tracked" in t:
        tr = t["tracked"]
        for mode, tag in (("default", "10 s deadline"), ("nodeadline", "no deadline")):
            for label, keep in (("31", lambda k: True), ("programs", lambda k: k == "program"), ("modules", lambda k: k == "module")):
                pooled = {s: heldout.pooled({n: x for n, x in tr[s].get(mode, {}).items() if keep(x["kind"])}) for s in sides}
                grp = f"tracked {label}, {tag}"
                add(grp, "wall geomean (s)", lambda s: pooled[s]["wall_geomean_s"], 2)
                add(grp, "RSS geomean / max (MB)", lambda s: f"{fmt(pooled[s]['rss_geomean_mb'], 0)} / {fmt(pooled[s]['rss_max_mb'])}")
                add(grp, "finished / cut", lambda s: f"{pooled[s]['finished']}/{pooled[s]['modules']} / {pooled[s]['cut']}")
                add(grp, "proven % / known facts %", lambda s: f"{fmt(pooled[s]['proven_pct'])} / {fmt(pooled[s]['known_facts_pct'])}")
                add(grp, "work (statements)", lambda s: pooled[s]["work"])
    else:
        skipped("tracked", "tracked", "the tracked set")
    if "heldout" in t:
        ho = t["heldout"]
        for mode, tag in (("default", "10 s deadline"), ("nodeadline", "no deadline")):
            pooled = {s: heldout.pooled(ho[s].get(mode, {})) for s in sides}
            grp = f"held-out, {tag}"
            add(grp, "wall geomean (s)", lambda s: pooled[s]["wall_geomean_s"], 2)
            add(grp, "RSS geomean / max (MB)", lambda s: f"{fmt(pooled[s]['rss_geomean_mb'], 0)} / {fmt(pooled[s]['rss_max_mb'])}")
            add(grp, "finished / cut", lambda s: f"{pooled[s]['finished']}/{pooled[s]['modules']} / {pooled[s]['cut']}")
            add(grp, "proven % / known facts %", lambda s: f"{fmt(pooled[s]['proven_pct'])} / {fmt(pooled[s]['known_facts_pct'])}")
    else:
        skipped("heldout", "held-out", "metrics")
    if "sitepackages" in t:
        sp = t["sitepackages"]

        def spf(s, what):
            fol = [m.get("follow", {}) for m in sp[s]["scenarios"].values()]
            if what == "gained":
                return f"{sum((x.get('vs_none') or {}).get('gained', 0) for x in fol)} / {sum((x.get('vs_none') or {}).get('lost', 0) for x in fol)}"
            return f"{sum(x.get('wall_s', 0) for x in fol):.1f} s, {sum(1 for x in fol if x.get('finished'))}/{len(fol)} finished, max {max((x.get('rss_mb') or 0) for x in fol)} MB"
        add("site-packages", "facts gained / lost (--follow '*')", lambda s: spf(s, "gained"))
        add("site-packages", "--follow '*': wall, finished, memory", lambda s: spf(s, "wall"))
    else:
        skipped("sitepackages", "site-packages", "bench/sitepackages.py")
    if "hovers" in t:
        hv = t["hovers"]
        for p in EIGHT:
            add("hovers", f"{p}: p50 / p99 / max ms; cut", lambda s, p=p: "{p50} / {p99} / {max}; {cut}".format(**hv[s][p]["hovers"]))
            add("hovers", f"{p}: first hover p50 / max ms; MB", lambda s, p=p: "{p50} / {max}; ".format(**hv[s][p]["first"]["first_hover_ms"]) + str(hv[s][p]["first"]["peak_mb"]))
    else:
        skipped("hovers", "hovers", "bench/hovers.py, first_hover.py")
    return out


def render(r: dict, sides: list[str], md: bool) -> str:
    rs = rows(r, sides)
    head = ["group", "metric", *sides]
    body = [[g, m, *[v[s] for s in sides]] for g, m, v in rs]
    if md:
        return "\n".join(["| " + " | ".join(head) + " |", "|" + "---|" * len(head)] + ["| " + " | ".join(map(str, b)) + " |" for b in body]) + "\n"
    w = [max(len(str(x[i])) for x in [head, *body]) for i in range(len(head))]
    return "\n".join("  ".join(str(c).ljust(w[i]) for i, c in enumerate(x)) for x in [head, *body]) + "\n"


# ---------------------------------------------------------------- main
def main() -> None:
    global GUARD
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--base", default="main", help="a git ref (built here), a binary, or none")
    ap.add_argument("--head", help="a binary (default: this checkout's, built)")
    ap.add_argument("--base-src", help="the ref whose source the base's code rows count, when --base is a binary")
    ap.add_argument("--only")
    ap.add_argument("--skip")
    ap.add_argument("--seeds", help="the fuzzer's two seeds (default: fresh)")
    ap.add_argument("-j", type=int, default=3, help="checks at once")
    ap.add_argument("--out")
    ap.add_argument("--fresh", action="store_true", help="ignore cached checks")
    ap.add_argument("--tags", help="then a scoreboard row for these tags (a scratch worktree only)")
    a = ap.parse_args()
    if a.tags and main_checkout() == ROOT:
        sys.exit("all.py: --tags writes bench/scoreboard.json: run it from a scratch worktree, not the main checkout")
    steps = [s for s in STEPS if (not a.only or s in a.only.split(",")) and (not a.skip or s not in a.skip.split(","))]
    t_start = time.monotonic()
    GUARD = setup_worktree()
    if GUARD is None and sys.platform == "darwin":
        print("all.py: no .guard/run here or in the main checkout: rowan runs unguarded", file=sys.stderr)
    out = Path(a.out) if a.out else WORK / time.strftime("%Y%m%d-%H%M%S")
    out.mkdir(parents=True, exist_ok=True)
    bins: dict[str, Path] = {}
    base_ref = None
    if a.base != "none":
        if Path(a.base).is_file():
            bins["base"] = Path(a.base).resolve()
            base_ref = a.base_src
        else:
            base_ref = a.base
            bins["base"] = build_ref(a.base)
    bins["head"] = Path(a.head).resolve() if a.head else build_head()
    sides = list(bins)
    seeds = [int(x) for x in a.seeds.split(",")] if a.seeds else [random.randrange(10_000, 1_000_000) for _ in range(2)]
    r: dict = {"seeds": seeds, "bins": {s: str(b) for s, b in bins.items()}, "steps_s": {}}
    print(f"all.py: {', '.join(f'{s} {b}' for s, b in bins.items())}; steps {', '.join(steps)}; out {out}", file=sys.stderr, flush=True)

    # checks, both binaries, a few at once (cached)
    fp = tests_fingerprint()
    cache = WORK / "cache"
    cache.mkdir(parents=True, exist_ok=True)

    def one(job):
        step, side = job
        key = cache / f"{step}-{sha_file(bins[side])[:16]}-{fp}{'-' + '-'.join(map(str, seeds)) if step == 'fuzz' else ''}.json"
        if key.exists() and not a.fresh:
            res = json.loads(key.read_text())
            res["cached"] = True
        else:
            print(f"[{step} {side}] …", file=sys.stderr, flush=True)
            res = check(step, bins[side], side, out, seeds)
            if "error" not in res:
                key.write_text(json.dumps(res))
        print(f"[{step} {side}] {json.dumps(res)[:200]}", file=sys.stderr, flush=True)
        return step, side, res

    jobs = [(st, s) for st in CHECKS if st in steps for s in sides]
    once_steps = [st for st in ONCE if st in steps]
    with ThreadPoolExecutor(a.j) as ex:
        futs = [ex.submit(one, j) for j in jobs]
        ofuts = {st: ex.submit(once, st, out) for st in once_steps}
        for f in futs:
            step, side, res = f.result()
            r.setdefault("checks", {}).setdefault(step, {})[side] = res
        for st, f in ofuts.items():
            r.setdefault("once", {})[st] = f.result()
    if "source" in steps:
        r["source"] = {s: source(base_ref if s == "base" else None) for s in sides if s == "head" or base_ref}
    # timing, alone
    for st in TIMING:
        if st in steps:
            print(f"[{st}] timing, interleaved", file=sys.stderr, flush=True)
            res = timing(st, bins, out)
            r["steps_s"][st] = res.pop("s")
            r.setdefault("timing", {})[st] = res
    r["total_s"] = round(time.monotonic() - t_start, 1)
    for st in ("checks", "once"):
        for step, v in r.get(st, {}).items():
            r["steps_s"][step] = sum(x.get("s", 0) for x in v.values()) if st == "checks" else v.get("s")
    (out / "all.json").write_text(json.dumps(r, indent=1, default=str))
    text = render(r, sides, False)
    md = render(r, sides, True)
    if "tracked" in r.get("timing", {}):
        import tracked

        tr = r["timing"]["tracked"]
        per = tracked.table(tr["head"], tr.get("base"), md=False)
        text += "\nThe tracked set, per program" + (" (base → head)" if "base" in tr else "") + ":\n" + per
        md += "\n" + tracked.table(tr["head"], tr.get("base"), md=True)
    text += "\nStep times (s, summed over binaries): " + ", ".join(f"{k} {v}" for k, v in r["steps_s"].items()) + f"\nTotal: {r['total_s'] / 60:.1f} min\n"
    (out / "table.txt").write_text(text)
    (out / "table.md").write_text(md)
    print(text)
    if a.tags:
        subprocess.run(py("bench/scoreboard.py", "run", "--tags", a.tags), cwd=ROOT, check=False)
    print(f"all.py: {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
