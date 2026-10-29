"""The progress scoreboard: every release of rowan measured with one fixed measurement set.

Each release tag (v0.1.N, one per merged pull request since #8) is built in a scratch worktree
and measured with the tests, fuzzer, benchmark drivers and site-packages scenarios of one pinned
commit (MEASURE_REF): newer tests find older bugs, so the numbers say how sound and precise each
release really was, not how it did on the suite of its day. Incremental: a release already in
the data file is not measured again (`--redo FAMILY` measures a family again; a new MEASURE_REF
means measuring everything again, as the old numbers used another set).

Families and their metrics (per release; `None` / missing: the release could not be measured so,
e.g. no such feature yet):

  soundness  fuzz      today's tests/fuzz.py at FUZZ_SEEDS (never used in CI, which runs seed 1),
                       FUZZ_N programs per kind and seed: checks, `unsound` (a claim CPython
                       contradicts: a value left out, a line taken for unreachable, a key set or
                       verdict claim), `crashed` (rowan failed: no answer), `timeouts` (over
                       CASE_TIMEOUT_S or CASE_MEMORY_MB), `unreadable` (output today's harness cannot read)
             golden    today's golden suites (tests/run_golden.py, PENDING cases left out):
                       passed / total, crashed, timeouts, unreadable, the failing cases
  precision  budget0   the golden suites with ROWAN_LIMITS=budget=0 (every call past its budget)
             bench.P   per benchmark program P (the `--facts` run below): verdict tiers (proven,
                       likely, possibly, likely-missing, missing, input, …: every read of the
                       program's code) and the main file's hover facts by level (bench/
                       sitepackages.py's `level`: `unknown` = `?` or only objects rowan does not
                       see; `partial` = a known shape with unknown parts, `int(?)`, `list[?]`, a
                       named unknown; `exact` = no unknown part; names of library modules,
                       classes and functions apart, `names`). The *known share* is
                       (partial + exact) / (unknown + partial + exact).
  speed      bench.P   one run of `rowan P --facts` with ROWAN_DEBUG=1 and no deadline
                       (ROWAN_LIMITS=ms=10^8), serially, cut at BENCH_TIMEOUT_S: wall time,
                       peak RSS (/usr/bin/time -l), the `work:` counters; `finished`
             sitepackages  today's bench/sitepackages.py (modes none, follow, only), releases
                       from FOLLOW_SINCE on (following installed packages began with PR #43)
  size       size      engine lines (rowan/src/**/*.rs outside models/) and model lines
  costset    costset   the cost set (bench/costset.py, bench/costset/manifest.json: 23 dense library
                       modules, tuned on), measured as heldout below, releases from COSTSET_SINCE
                       on; with the eight it is the
                       *tracked* (tuned-on) set of 31 the main metrics pool over (`indices`)
  protocols  protocols today's protocol enumerators (PROTOCOLS: tests/try_protocol.py, gen_, with_,
                       iter_, call_, rec_, method_, closure_, narrow_, dunder_, anycall_, str_,
                       lib_, container_, loop_, class_protocol.py, the pair modes (`--pairs`) of narrow_,
                       dunder_, class_, container_, call_protocol.py; collections_, itertools_, sqlite_, config_,
                       argparse_, pathlib_, async_, descriptor_, deferred_, csv_, pandas_, pydantic_,
                       tensor_protocol.py, at their CI bounds) run with the release's binary: per
                       enumerator the points checked and those where rowan's claims miss a CPython run;
                       the enumerators' sha256 (changed enumerators are another series)
  heldsound  heldsound bench/heldout.py sound (the held-out corpus's own tests against rowan's claims,
                       ~25 min) from HELDSOUND_SINCE on: candidate sites, and those not in
                       bench/heldout/sound_baseline.json (`new`)
  heldout    heldout   the held-out corpus (bench/heldout.py, bench/heldout/manifest.json: 31 real
                       packages no iteration tunes against, 52 modules run as main files), measured
                       like bench.P: with no deadline, cut at heldout.NODEADLINE_TIMEOUT_S, serially;
                       pooled proven / possibly %, known facts %, wall and RSS geomeans, finished,
                       cut, skipped calls; the manifest's sha256 (a changed corpus is another set)

The *wrong claims* (`indices`' `wrong`) pool the places where rowan's claims miss real Python:
fuzz unsound checks at the fresh seeds, held-out `sound` sites not in the baseline, and the
protocol enumerators' missed points; a source not measured for a release is listed in
`missing` (the total then a lower bound), not counted as 0.

The headline metrics (proven %, possibly %, known facts %, wall and RSS geomeans, cuts) pool the
tracked set (the eight + the cost set); the eight's own numbers stay beside them (`precision`,
`speed`: comparable with the releases measured before the cost set). The *combined* headline
pools tracked + held-out; the gap between the two (tracked minus held-out) is the overfitting
signal. A rotation (bench/heldout.py rotate, the owner's) moves the held-out modules into the
cost set's manifest: both manifests' hashes change, so the series start again there.

Every rowan run goes through .guard/run (the harnesses' runs too: the harness runs under it).
Non-timing families run up to `-j` releases at once (the guard's three slots); timing runs one
at a time, nothing else running.

Usage (run from anywhere, under `uv run --no-project python`):
  python bench/scoreboard.py run [--tags v0.1.8,v0.1.9|--since v0.1.40] [--ref REF…]
         [--families soundness,precision,speed,size,sitepackages,costset,heldout] [--redo FAMILY,…] [-j 3]
         [--cache DIR]
  python bench/scoreboard.py repeat --tags A,B [-n 3]   timing noise: the benchmarks again
  python bench/scoreboard.py report [--md OUT.md]        per-release deltas, top iterations,
                                                         flattening, the stop criterion
`--ref REF` also measures a commit that has no tag yet (labelled by its short hash).
Cache (binaries, worktrees, the pinned measurement set, raw outputs): bench/work/scoreboard/
(gitignored) unless --cache. The results: bench/scoreboard.json (committed).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
GUARD = ROOT / ".guard" / "run"
DATA = HERE / "scoreboard.json"
# the measurement set: tests/, bench/drivers/, bench/sitepackages* of this commit (main after #79)
MEASURE_REF = "5aaa094fcfdb82c6f59c0936ef6358423204a547"
FUZZ_SEEDS = (1001, 2001, 3001)
FUZZ_N = 100
CASE_TIMEOUT_S = 60  # per golden case / fuzz program
CASE_MEMORY_MB = 3000  # likewise (several run at once under the guard's 16 GB for all)
BENCH_TIMEOUT_S = 180
PROGRAMS = ["stdlib12k", "email", "rich", "click", "pip", "timm", "sktime", "functional"]
FOLLOW_SINCE = 40  # v0.1.40: PR #43, following installed packages
COSTSET_SINCE = 80  # the cost set measured from v0.1.80 on (PR #89: older releases cost too much to measure)
HELDSOUND_SINCE = 86  # bench/heldout.py sound (~25 min a release) measured from v0.1.86 on
# the protocol enumerators and their arguments (their CI bounds)
PROTOCOLS = {"try": ["tests/try_protocol.py", "--rowan", "2000"], "gen": ["tests/gen_protocol.py", "--rowan", "2000"],
             "with": ["tests/with_protocol.py", "--rowan", "2000"], "iter": ["tests/iter_protocol.py", "--rowan", "2000"],
             "call": ["tests/call_protocol.py", "--rowan", "2000"], "rec": ["tests/rec_protocol.py", "--rowan", "1500"],
             "method": ["tests/method_protocol.py", "--rowan", "1500"], "closure": ["tests/closure_protocol.py", "--rowan", "2000"],
             "narrow": ["tests/narrow_protocol.py", "--rowan", "5000"], "dunder": ["tests/dunder_protocol.py", "--all"],
             "anycall": ["tests/anycall_protocol.py", "--rowan", "3000"], "str": ["tests/str_protocol.py", "--all"],
             "lib": ["tests/lib_protocol.py", "--all"], "container": ["tests/container_protocol.py", "--all"],
             "loop": ["tests/loop_protocol.py", "--all"], "class": ["tests/class_protocol.py", "--all"],
             "narrow-pairs": ["tests/narrow_protocol.py", "--pairs", "--rowan", "5000"],
             "dunder-pairs": ["tests/dunder_protocol.py", "--pairs", "--rowan", "2500"],
             "class-pairs": ["tests/class_protocol.py", "--pairs", "--rowan", "2500"],
             "container-pairs": ["tests/container_protocol.py", "--pairs", "--rowan", "4000"],
             "call-pairs": ["tests/call_protocol.py", "--pairs", "--rowan", "1500"],
             "collections": ["tests/collections_protocol.py", "--all"],
             "pathlib": ["tests/pathlib_protocol.py", "--all"],
             "argparse": ["tests/argparse_protocol.py", "--all"],
             "configparser": ["tests/config_protocol.py", "--all"],
             "sqlite": ["tests/sqlite_protocol.py", "--all"],
             "itertools": ["tests/itertools_protocol.py", "--all"],
             "async": ["tests/async_protocol.py", "--all"],
             "descriptor": ["tests/descriptor_protocol.py", "--all"],
             "deferred": ["tests/deferred_protocol.py", "--rowan", "2000"],
             "csv": ["tests/csv_protocol.py", "--all"],
             "pandas": ["tests/pandas_protocol.py", "--all"],
             "pydantic": ["tests/pydantic_protocol.py", "--all"],
             "tensor": ["tests/tensor_protocol.py", "--all"]}
# the libraries an enumerator runs real code of (`uv run --with`)
PROTOCOL_DEPS = {"pandas": ["pandas"], "pydantic": ["pydantic"], "tensor": ["torch", "numpy"]}
FAMILIES = ["size", "soundness", "precision", "speed", "sitepackages", "costset", "heldout", "protocols", "heldsound"]
LOCK = threading.RLock()


# ---------------------------------------------------------------- the harness shim
# Today's harnesses read what today's binary writes; an old binary's golden document may lack a
# field they need. Under SCOREBOARD_TESTS (set for `_harness` and inherited by the fuzzer's
# worker processes, which import this module as their main one) a case whose output the harness
# cannot read fails as `scoreboard: unreadable`, not with an exception that stops the whole run.
def _install_shim(tests: str) -> None:
    sys.path.insert(0, tests)
    import run_golden

    orig = run_golden.run_case

    def safe(path, verbose=False):
        try:
            return orig(path, verbose)
        except Exception as e:  # noqa: BLE001
            return False, [f"scoreboard: unreadable output ({type(e).__name__}: {e})"]

    run_golden.run_case = safe
    import fuzz

    fuzz.UNSOUND = tuple(fuzz.UNSOUND) + ("scoreboard: unreadable",)


if os.environ.get("SCOREBOARD_TESTS"):
    _install_shim(os.environ["SCOREBOARD_TESTS"])


def harness_main(kind: str, args: list[str]) -> int:
    if kind == "golden":
        import run_golden

        return run_golden.main(args)
    import fuzz

    sys.argv = ["fuzz.py", *args]
    return fuzz.main()


# ---------------------------------------------------------------- releases and builds
def git(*args: str, cwd: Path = ROOT) -> str:
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def releases(tags: list[str] | None, since: str | None, refs: list[str]) -> list[dict]:
    """The releases in order, each with its commit, date, engine tree and the pull requests new
    in it (merges reachable from it and not from the release before)."""
    all_tags = sorted(git("tag", "--list", "v*").split(), key=lambda t: [int(x) for x in t[1:].split(".")])
    out, prev_prs = [], set()
    pr_titles = load_pr_titles()
    for label in all_tags + refs:
        commit = git("rev-parse", f"{label}^{{commit}}")
        merges = git("log", "--merges", "--format=%s", commit)
        prs = {int(m) for m in re.findall(r"Merge pull request #(\d+)", merges)}
        new = sorted(prs - prev_prs)
        prev_prs = prs
        name = label if label in all_tags else commit[:7]
        out.append({
            "tag": name,
            "commit": commit,
            "date": git("log", "-1", "--format=%cI", commit),
            "tree": git("rev-parse", f"{commit}:rowan"),
            "prs": [{"n": n, "title": pr_titles.get(n, "")} for n in new],
        })
    if tags:
        out = [r for r in out if r["tag"] in tags or r["commit"].startswith(tuple(tags))]
    if since:
        i = next(i for i, r in enumerate(out) if r["tag"] == since)
        out = out[i:]
    return out


def load_pr_titles() -> dict[int, str]:
    try:
        p = subprocess.run(["gh", "pr", "list", "--state", "merged", "--limit", "500", "--json", "number,title"],
                           cwd=ROOT, capture_output=True, text=True, timeout=60)
        return {d["number"]: d["title"] for d in json.loads(p.stdout)}
    except Exception:  # noqa: BLE001
        old = load_data()
        return {p["n"]: p["title"] for r in old["releases"] for p in r.get("prs", [])}


def build(rel: dict, cache: Path) -> Path | None:
    """The release's binary (cached by the tree of rowan/: releases with the same engine share it)."""
    out = cache / "bin" / rel["tree"][:12] / "rowan"
    if out.exists():
        return out
    wt = cache / "wt"
    if not wt.exists():
        git("worktree", "add", "--detach", str(wt), rel["commit"])
    else:
        git("checkout", "-q", "--detach", "--force", rel["commit"], cwd=wt)
        git("clean", "-q", "-fdx", "-e", "rowan/Cargo.lock", cwd=wt)
    # (Cargo.lock is not committed: today's keeps the dependencies' versions fixed)
    lock = ROOT / "rowan" / "Cargo.lock"
    if lock.exists() and not (wt / "rowan" / "Cargo.lock").exists():
        shutil.copy(lock, wt / "rowan" / "Cargo.lock")
    env = dict(os.environ, CARGO_TARGET_DIR=str(cache / "target"))
    print(f"[{rel['tag']}] building", file=sys.stderr, flush=True)
    p = subprocess.run([str(GUARD), "-t", "0", "cargo", "build", "--release", "--manifest-path", str(wt / "rowan" / "Cargo.toml")],
                       env=env, capture_output=True, text=True)
    if p.returncode != 0:
        print(f"[{rel['tag']}] build failed:\n{p.stderr[-2000:]}", file=sys.stderr)
        return None
    out.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy(cache / "target" / "release" / "rowan", out)
    return out


WRAPPER = r"""#!/usr/bin/perl
# rowan, killed past %(t)d s or %(mb)d MB of RSS (an old release may blow up on a newer test)
my $pid = fork() // die;
if (!$pid) { exec('%(bin)s', @ARGV) or die; }
my ($t0, $n) = (time, 0);
while (1) {
  my $r = waitpid($pid, 1);
  if ($r == $pid) { exit($? & 127 ? 128 + ($? & 127) : $? >> 8); }
  select(undef, undef, undef, $n++ < 20 ? 0.01 : 0.2);
  next if $n < 20;
  my $rss = `ps -o rss= -p $pid` + 0;
  if ($rss > %(mb)d * 1024 || time - $t0 > %(t)d) { kill 9, $pid; waitpid($pid, 0); kill 9, $$; }
}
"""


def with_timeout(binary: Path) -> Path:
    """A wrapper that kills the binary after CASE_TIMEOUT_S or past CASE_MEMORY_MB, and then
    itself by SIGKILL (the harness sees `exit -9`: a cut case)."""
    w = binary.parent / "rowan-t"
    text = WRAPPER % {"t": CASE_TIMEOUT_S, "mb": CASE_MEMORY_MB, "bin": binary}
    if not w.exists() or w.read_text() != text:
        w.write_text(text)
        w.chmod(0o755)
    return w


def measure_set(cache: Path) -> Path:
    """MEASURE_REF's tests/ and bench/ (drivers, site-packages scenarios), extracted once; its
    bench/work and .guard are the repository's (the prepared programs, the venvs, the guard)."""
    d = cache / f"measure-{MEASURE_REF[:10]}"
    if not (d / "tests" / "run_golden.py").exists():
        d.mkdir(parents=True, exist_ok=True)
        tar = subprocess.run(["git", "archive", MEASURE_REF, "tests", "bench"], cwd=ROOT, check=True, capture_output=True).stdout
        subprocess.run(["tar", "-x", "-C", str(d)], input=tar, check=True)
        shutil.rmtree(d / "bench" / "work", ignore_errors=True)
        (d / "bench" / "work").symlink_to(ROOT / "bench" / "work")
        (d / ".guard").symlink_to(ROOT / ".guard")
    return d


# ---------------------------------------------------------------- data
def load_data() -> dict:
    if DATA.exists():
        return json.loads(DATA.read_text())
    return {"releases": []}


def save_data(data: dict) -> None:
    with LOCK:
        for r in data["releases"]:
            # (the family indices, derived: what the Pages overview plots)
            r["index"] = indices(r)
        tmp = DATA.with_suffix(".tmp")
        # (one release per line: a new release is a one-line diff)
        head = {k: v for k, v in data.items() if k != "releases"}
        rels = ",\n".join(json.dumps(r, ensure_ascii=False, separators=(",", ":")) for r in data["releases"])
        tmp.write_text(json.dumps(head, ensure_ascii=False)[:-1] + ',\n"releases":[\n' + rels + "\n]}\n")
        tmp.replace(DATA)


def put(data: dict, e: dict, key: str, value) -> None:
    with LOCK:
        e[key] = value
        save_data(data)


def entry(data: dict, rel: dict) -> dict:
    with LOCK:
        for r in data["releases"]:
            if r["tag"] == rel["tag"]:
                r.update({k: rel[k] for k in ("commit", "date", "tree", "prs")})
                return r
        data["releases"].append(dict(rel))
        data["releases"].sort(key=lambda r: r["date"])
        return next(r for r in data["releases"] if r["tag"] == rel["tag"])


def failed(r) -> bool:
    """Not measured, or the measurement itself failed (an error, a fuzz seed without checks)."""
    if not r or "error" in r:
        return True
    return any("error" in x for x in r.get("seeds", {}).values()) or r.get("checks") == 0


def same_engine(data: dict, rel: dict, family: str) -> dict | None:
    """A family measured on another release with the same engine (the same rowan/ tree)."""
    for r in data["releases"]:
        if r["tree"] == rel["tree"] and r["tag"] != rel["tag"] and not failed(r.get(family)):
            return r[family]
    return None


# ---------------------------------------------------------------- families
def measure_size(rel: dict) -> dict:
    files = git("ls-tree", "-r", "--name-only", rel["commit"], "rowan/src").split()
    engine = models = 0
    for f in files:
        if not f.endswith(".rs"):
            continue
        n = git("show", f"{rel['commit']}:{f}").count("\n") + 1
        if "/models/" in f:
            models += n
        else:
            engine += n
    return {"engine_lines": engine, "model_lines": models}


def run_harness(binary: Path, mset: Path, kind: str, args: list[str], limits: str | None, log: Path) -> str:
    env = dict(os.environ, ROWAN_BIN=str(with_timeout(binary)), SCOREBOARD_TESTS=str(mset / "tests"))
    env.pop("ROWAN_LIMITS", None)
    if limits:
        env["ROWAN_LIMITS"] = limits
    cmd = [str(GUARD), "-t", "0", sys.executable, str(Path(__file__).resolve()), "_harness", kind, *args]
    with open(log, "w") as f:
        subprocess.run(cmd, env=env, stdout=f, stderr=subprocess.STDOUT, cwd=mset)
    return log.read_text(errors="replace")


def why(problems: list[str]) -> str:
    text = " ".join(problems)
    if "scoreboard: unreadable" in text:
        return "unreadable"
    if "exit -9)" in text or "exit -14)" in text:
        return "timeouts"
    if any(p.startswith("rowan failed") or p.startswith("bad JSON") for p in problems):
        return "crashed"
    return "failed"


def measure_golden(binary: Path, mset: Path, limits: str | None, log: Path) -> dict:
    out = run_harness(binary, mset, "golden", [], limits, log)
    summary = out[out.rfind("===== summary =====") :]
    m = re.search(r"^\s*(\d+)/(\d+) passed", summary, re.M)
    if not m:
        return {"error": out[-500:]}
    fails, cur = {}, None
    for line in summary.splitlines():
        if line.startswith("  FAIL  "):
            cur = line[8:].strip()
            fails[cur] = []
        elif line.startswith("  PASS  "):
            cur = None
        elif cur and line.startswith("        - "):
            fails[cur].append(line[10:])
    c = Counter(why(p) for p in fails.values())
    return {"passed": int(m.group(1)), "total": int(m.group(2)), "crashed": c["crashed"], "timeouts": c["timeouts"],
            "unreadable": c["unreadable"], "failing": sorted(fails)}


def measure_fuzz(binary: Path, mset: Path, logdir: Path) -> dict:
    res = {"checks": 0, "unsound": 0, "crashed": 0, "timeouts": 0, "unreadable": 0, "kinds": Counter(), "seeds": {}}
    for seed in FUZZ_SEEDS:
        work = Path(tempfile.mkdtemp(prefix=f"fuzz{seed}-", dir=logdir))
        out = run_harness(binary, mset, "fuzz", ["-n", str(FUZZ_N), "--seed", str(seed), "-j", "3", "--out", str(work)], None, logdir / f"fuzz{seed}.log")
        shutil.rmtree(work, ignore_errors=True)
        m = re.search(r"^(\d+)/(\d+) checks agree", out, re.M)
        if not m:
            res["seeds"][str(seed)] = {"error": out[-500:]}
            continue
        blocks, cur = [], None
        for line in out.splitlines():
            if line.startswith("FAIL "):
                cur = [line[5:], []]
                blocks.append(cur)
            elif cur and line.startswith("    ") and not line.startswith("    shrunk"):
                cur[1].append(line.strip())
            else:
                cur = None
        c = Counter()
        for path, probs in blocks:
            w = why(probs)
            w = "unsound" if w == "failed" else w
            c[w] += 1
            if w == "unsound":
                res["kinds"][Path(path).name.split("_")[0]] += 1
        checks = int(m.group(2))
        res["seeds"][str(seed)] = {"checks": checks, **{k: c[k] for k in ("unsound", "crashed", "timeouts", "unreadable")}}
        res["checks"] += checks
        for k in ("unsound", "crashed", "timeouts", "unreadable"):
            res[k] += c[k]
    res["kinds"] = dict(res["kinds"])
    return res


WORK_FIELDS = {"work": r"^work: (\d+)", "units": r"units (\d+)", "tabulations": r"tabulations (\d+)",
               "objects": r"objects (\d+)", "row_joins": r"row joins (\d+)", "havoc": r"havoc visits (\d+)"}


def facts_summary(doc: dict) -> dict:
    sys.path.insert(0, str(HERE))
    from sitepackages import is_name, level  # the levels' one definition

    tiers = Counter(v.get("tier", "?") for v in doc.get("verdicts", []))
    lv = Counter()
    for f in doc.get("facts", []):
        try:
            lv["names" if is_name(f) else ["unknown", "partial", "exact"][level(f)]] += 1
        except (KeyError, TypeError):
            lv["unreadable"] += 1
    return {"tiers": dict(tiers), "facts": dict(lv)}


def bench_file(name: str) -> Path:
    return next((ROOT / "bench" / "work" / name).glob("*.py")) if name == "stdlib12k" else ROOT / "bench" / "work" / name / "main.py"


def measure_bench(binary: Path, name: str, logdir: Path) -> dict:
    """One run of the program with --facts, timed (call it with nothing else running)."""
    out, err = logdir / f"{name}.facts.json", logdir / f"{name}.err"
    env = dict(os.environ, ROWAN_DEBUG="1", ROWAN_LIMITS="ms=100000000")
    cmd = [str(GUARD), "-t", str(BENCH_TIMEOUT_S), "/usr/bin/time", "-l", str(binary), str(bench_file(name)), "--facts"]
    t0 = time.monotonic()
    with open(out, "w") as so, open(err, "w") as se:
        p = subprocess.run(cmd, stdout=so, stderr=se, env=env, cwd=bench_file(name).parent)
    r = {"finished": p.returncode == 0, "wall_s": round(time.monotonic() - t0, 2)}
    for line in err.read_text(errors="replace").splitlines():
        if line.startswith("work: "):
            for k, pat in WORK_FIELDS.items():
                if m := re.search(pat, line):
                    r[k] = int(m.group(1))
        elif m := re.match(r"^\s*([\d.]+) real", line):
            r["wall_s"] = float(m.group(1))
        elif "maximum resident set size" in line:
            r["rss_mb"] = round(int(line.split()[0]) / 2**20)
        elif line.startswith("GUARD"):
            r["why"] = line
    if not r["finished"]:
        r.setdefault("why", f"exit {p.returncode}")
        if "over" in r["why"]:
            r["wall_s"] = BENCH_TIMEOUT_S
    else:
        try:
            doc = json.loads(out.read_text())
            r.update(facts_summary(doc))
            r["cut"] = bool(doc.get("analysis", {}).get("cut"))
        except json.JSONDecodeError:
            r["facts_doc"] = "unreadable"
    out.unlink(missing_ok=True)
    gz = err.with_suffix(".err.tail")
    gz.write_text("\n".join(err.read_text(errors="replace").splitlines()[-40:]))
    err.unlink()
    return r


def measure_sitepackages(binary: Path, mset: Path, log: Path) -> dict:
    out = log.with_suffix(".json")
    cmd = [sys.executable, str(mset / "bench" / "sitepackages.py"), "--rowan", str(binary), "--json", "--no-check", "--out", str(out)]
    with open(log, "w") as f:
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=f, cwd=ROOT)
    if not out.exists():
        return {"error": log.read_text(errors="replace")[-500:]}
    doc = json.loads(out.read_text())
    keep = ("finished", "wall_s", "rss_mb", "statements", "levels", "vs_none", "why")
    return {s: {m: {k: v for k, v in r.items() if k in keep} for m, r in modes.items()} for s, modes in doc["scenarios"].items()}


def measure_heldout(binary: Path, logdir: Path) -> dict:
    """The held-out corpus with today's bench/heldout.py and manifest (not MEASURE_REF's: the
    corpus is pinned by its own manifest, whose hash is kept), no deadline, serially."""
    import heldout

    res = heldout.metrics(binary, "nodeadline", None, None, 1, logdir / "heldout")
    return {"manifest_sha256": hashlib.sha256(heldout.MANIFEST.read_bytes()).hexdigest(), "pooled": res["pooled"], "by_kind": res["by_kind"],
            "runs": {m: {k: v for k, v in r.items() if k in KEEP} for m, r in res["runs"].items()}}


KEEP = ("finished", "wall_s", "rss_mb", "cut", "skipped_calls", "work", "tiers", "facts", "why")


def measure_costset(binary: Path, logdir: Path) -> dict:
    """The cost set with today's bench/costset.py and manifest, no deadline, serially."""
    import costset

    res = costset.metrics(binary, "nodeadline", None, None, 1, logdir / "costset")
    return {"manifest_sha256": hashlib.sha256(costset.MANIFEST.read_bytes()).hexdigest(), "pooled": res["pooled"],
            "runs": {m: {k: v for k, v in r.items() if k in KEEP} for m, r in res["runs"].items()}}


def protocols_sha() -> str:
    h = hashlib.sha256()
    for f in sorted({ROOT / "tests" / "protocol_lib.py", *(ROOT / argv[0] for argv in PROTOCOLS.values())}):
        h.update(f.read_bytes())
    return h.hexdigest()


def measure_protocols(binary: Path, logdir: Path) -> dict:
    """Today's protocol enumerators (PROTOCOLS) with the release's binary: per enumerator the
    points checked and missed (None: it could not run)."""
    out: dict = {"sha256": protocols_sha()}
    for name, argv in PROTOCOLS.items():
        deps = [x for d in PROTOCOL_DEPS.get(name, []) for x in ("--with", d)]
        r = subprocess.run(["uv", "run", "--no-project", *deps, "python", str(ROOT / argv[0]), *argv[1:]], cwd=ROOT, capture_output=True, text=True,
                           env={**os.environ, "ROWAN_BIN": str(binary)}, stdin=subprocess.DEVNULL)
        (logdir / f"protocol-{name}.log").write_text(r.stdout + r.stderr)
        # (one line per limit set an enumerator runs under, `rowan (ROWAN_LIMITS=…):`: summed)
        lines = re.findall(r"^rowan(?: \(ROWAN_LIMITS=[^)]*\))?: .*?(\d+) points checked, (\d+) with a run rowan's claims miss.*rowan failed on (\d+) batches", r.stdout, re.M)
        points, missed, failed = (sum(int(x[i]) for x in lines) for i in range(3))
        # (a rowan that could not answer a batch, or claimed nothing anywhere — one without
        # `#@general` rows: not measured)
        ok = lines and not failed and missed < points
        out[name] = {"points": points, "missed": missed} if ok else None
    return out


def measure_heldsound(binary: Path, logdir: Path) -> dict:
    """bench/heldout.py sound with the release's binary: candidate sites, and those not in the
    baseline (`new`)."""
    import heldout

    js = logdir / "heldsound.json"
    r = subprocess.run(["uv", "run", "--no-project", "python", str(HERE / "heldout.py"), "sound", "--rowan", str(binary), "--json", str(js)],
                       cwd=ROOT, capture_output=True, text=True, stdin=subprocess.DEVNULL)
    (logdir / "heldsound.log").write_text(r.stdout + r.stderr)
    if not js.exists():
        return {"error": r.stderr[-500:]}
    sites = heldout.candidate_sites(json.loads(js.read_text()))
    known = json.loads(heldout.BASELINE.read_text())["sites"]
    return {"sites": len(sites), "new": sum(1 for k in sites if k not in known)}


# ---------------------------------------------------------------- run
def cmd_run(a) -> None:
    cache = Path(a.cache).resolve()
    cache.mkdir(parents=True, exist_ok=True)
    data = load_data()
    if data.get("measure_ref") not in (None, MEASURE_REF):
        sys.exit(f"bench/scoreboard.json was measured with {data['measure_ref'][:10]}, not MEASURE_REF {MEASURE_REF[:10]}: "
                 "measure everything again (--redo all) or set MEASURE_REF back")
    data.update(measure_ref=MEASURE_REF, fuzz_seeds=list(FUZZ_SEEDS), fuzz_n=FUZZ_N, bench_timeout_s=BENCH_TIMEOUT_S,
                case_timeout_s=CASE_TIMEOUT_S, machine=machine())
    rels = releases(a.tags.split(",") if a.tags else None, a.since, a.ref or [])
    fams = a.families.split(",") if a.families else FAMILIES
    redo = set(FAMILIES if a.redo == "all" else (a.redo.split(",") if a.redo else []))
    keys = {"soundness": ["golden", "fuzz"], "precision": ["budget0"], "speed": ["bench"], "sitepackages": ["sitepackages"], "size": ["size"], "costset": ["costset"],
            "heldout": ["heldout"], "protocols": ["protocols"], "heldsound": ["heldsound"]}

    def todo(e: dict, fam: str) -> bool:
        if (fam == "costset" and not since(e, COSTSET_SINCE)) or (fam == "heldsound" and not since(e, HELDSOUND_SINCE)):
            return False
        # (the protocol enumerators changed: measured again)
        if fam == "protocols" and fam in fams and (e.get("protocols") or {}).get("sha256") not in (None, protocols_sha()):
            return True
        return fam in fams and (fam in redo or any(failed(e.get(k)) for k in keys[fam]))

    mset = measure_set(cache)
    for rel in rels:
        e = entry(data, rel)
        if todo(e, "size"):
            e["size"] = measure_size(rel)
    save_data(data)
    binaries = {}

    def logdir(rel):
        d = cache / "logs" / rel["tag"]
        d.mkdir(parents=True, exist_ok=True)
        return d

    def nontiming(rel):
        e, b = entry(data, rel), binaries.get(rel["tag"])
        if b is None:
            return
        if todo(e, "soundness"):
            for k, f in (("golden", lambda: measure_golden(b, mset, None, logdir(rel) / "golden.log")),
                         ("fuzz", lambda: measure_fuzz(b, mset, logdir(rel)))):
                if not failed(e.get(k)) and "soundness" not in redo:
                    continue
                print(f"[{rel['tag']}] {k}", file=sys.stderr, flush=True)
                put(data, e, k, same_engine(data, rel, k) or f())
        if todo(e, "protocols"):
            print(f"[{rel['tag']}] protocols", file=sys.stderr, flush=True)
            put(data, e, "protocols", measure_protocols(b, logdir(rel)))
        if todo(e, "precision"):
            print(f"[{rel['tag']}] budget0", file=sys.stderr, flush=True)
            r = dict(same_engine(data, rel, "budget0") or measure_golden(b, mset, "budget=0", logdir(rel) / "budget0.log"))
            r.pop("failing", None)
            put(data, e, "budget0", r)

    # (builds one at a time, each release's non-timing families measured as soon as it is built)
    with ThreadPoolExecutor(a.j) as ex:
        jobs = []
        for rel in rels:
            e = entry(data, rel)
            if any(todo(e, f) for f in fams if f != "size"):
                binaries[rel["tag"]] = build(rel, cache)
                jobs.append(ex.submit(nontiming, rel))
        for j in jobs:
            j.result()
    # timing: one run at a time
    for rel in rels:
        e, b = entry(data, rel), binaries.get(rel["tag"])
        if b is None:
            continue
        if todo(e, "speed"):
            res = {}
            for name in PROGRAMS:
                print(f"[{rel['tag']}] bench {name}", file=sys.stderr, flush=True)
                res[name] = measure_bench(b, name, logdir(rel))
            put(data, e, "bench", res)
        if todo(e, "sitepackages") and follows(rel):
            print(f"[{rel['tag']}] sitepackages", file=sys.stderr, flush=True)
            put(data, e, "sitepackages", measure_sitepackages(b, mset, logdir(rel) / "sitepackages.log"))
        if todo(e, "costset"):
            print(f"[{rel['tag']}] costset", file=sys.stderr, flush=True)
            put(data, e, "costset", measure_costset(b, logdir(rel)))
        if todo(e, "heldout"):
            print(f"[{rel['tag']}] heldout", file=sys.stderr, flush=True)
            put(data, e, "heldout", measure_heldout(b, logdir(rel)))
        if todo(e, "heldsound"):
            print(f"[{rel['tag']}] heldsound", file=sys.stderr, flush=True)
            put(data, e, "heldsound", measure_heldsound(b, logdir(rel)))
    save_data(data)


def follows(rel: dict) -> bool:
    """Whether the release can follow installed packages (an untagged commit: a recent one)."""
    return since(rel, FOLLOW_SINCE)


def since(rel: dict, n: int) -> bool:
    """Is the release v0.1.N or later (an untagged commit: a recent one)?"""
    return not rel["tag"].startswith("v") or int(rel["tag"].rsplit(".", 1)[-1]) >= n


def cmd_repeat(a) -> None:
    cache = Path(a.cache).resolve()
    data = load_data()
    for rel in releases(a.tags.split(","), None, a.ref or []):
        e, b = entry(data, rel), build(rel, cache)
        reps = e.setdefault("bench_repeats", [])
        for _ in range(a.n):
            d = cache / "logs" / rel["tag"] / "repeat"
            d.mkdir(parents=True, exist_ok=True)
            reps.append({name: {k: v for k, v in measure_bench(b, name, d).items() if k in ("finished", "wall_s", "rss_mb", "work")} for name in PROGRAMS})
            save_data(data)


def machine() -> str:
    try:
        chip = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True).stdout.strip()
    except OSError:
        chip = ""
    return f"{chip or sys.platform}, {os.cpu_count()} cores"


# ---------------------------------------------------------------- the indices and the report
def geomean(xs: list[float]) -> float | None:
    xs = [x for x in xs if x and x > 0]
    return math.exp(sum(math.log(x) for x in xs) / len(xs)) if xs else None


def indices(e: dict) -> dict:
    """The family indices of one release (None: not measured):
    soundness  golden pass %, fuzz agree % (100 - unsound per 100 checks), and their mean
    precision  budget=0 pass %; proven share of the benchmarks' verdicts (pooled); known share of
               their main files' hover facts (pooled); `index`: their mean
    speed      geometric means over the eight programs of wall time (s; a cut run counts as its
               timeout) and peak memory (MB); how many finished
    size       engine and model lines
    tracked    the eight + the cost set (31 runs), pooled (`pool`): proven / possibly / known facts %,
               wall and RSS geomeans, finished, cuts (runs the memory bound or the timeout cut,
               or whose analysis was cut); None until both are measured
    heldout    the held-out corpus, likewise
    combined   tracked + held-out pooled; `gap`: tracked minus held-out (shares: points; time
               and memory: the ratio), the overfitting signal"""
    out = {}
    g, f = e.get("golden", {}), e.get("fuzz", {})
    gp = 100 * g["passed"] / g["total"] if g.get("total") else None
    fz = 100 * (1 - f["unsound"] / f["checks"]) if f.get("checks") else None
    out["soundness"] = {"golden_pct": gp, "fuzz_agree_pct": fz, "unsound_per_10k": 1e4 * f["unsound"] / f["checks"] if f.get("checks") else None,
                        "fuzz_unsound": f.get("unsound"), "fuzz_crashed": f.get("crashed"), "golden_passed": g.get("passed"),
                        "index": (gp + fz) / 2 if gp is not None and fz is not None else None}
    b0 = e.get("budget0", {})
    bp = 100 * b0["passed"] / b0["total"] if b0.get("total") else None
    bench = e.get("bench", {})
    tiers, facts = Counter(), Counter()
    for r in bench.values():
        if r.get("finished"):
            tiers.update(r.get("tiers", {}))
            facts.update(r.get("facts", {}))
    nt = sum(tiers.values())
    nf = facts["unknown"] + facts["partial"] + facts["exact"]
    proven = 100 * tiers["proven"] / nt if nt else None
    known = 100 * (facts["partial"] + facts["exact"]) / nf if nf else None
    parts = [x for x in (bp, proven, known) if x is not None]
    out["precision"] = {"budget0_pct": bp, "budget0_passed": b0.get("passed"), "proven_pct": proven, "possibly_pct": 100 * tiers["possibly"] / nt if nt else None,
                        "known_facts_pct": known, "exact_facts_pct": 100 * facts["exact"] / nf if nf else None, "verdicts": nt, "facts": nf,
                        "index": sum(parts) / len(parts) if len(parts) == 3 else None}
    walls = [r.get("wall_s") for r in bench.values()]
    rss = [r.get("rss_mb") for r in bench.values() if r.get("finished")]
    out["speed"] = {"wall_geomean_s": geomean(walls) if len(walls) == len(PROGRAMS) else None, "rss_geomean_mb": geomean(rss) if len(rss) == len(PROGRAMS) else None,
                    "finished": sum(1 for r in bench.values() if r.get("finished")) if bench else None,
                    "work_geomean": geomean([r.get("work") for r in bench.values() if r.get("finished")]) if bench else None}
    s = e.get("size", {})
    out["size"] = {"engine_lines": s.get("engine_lines"), "model_lines": s.get("model_lines")}
    ho = (e.get("heldout") or {}).get("pooled")
    if ho:
        out["heldout"] = {k: ho.get(k) for k in ("proven_pct", "possibly_pct", "known_facts_pct", "wall_geomean_s", "rss_geomean_mb", "finished", "modules", "cut", "skipped_calls")}
    tracked = tracked_runs(e)
    held = {"heldout/" + m: r for m, r in ((e.get("heldout") or {}).get("runs") or {}).items()}
    if tracked:
        out["tracked"] = pool(tracked)
    if held:
        out["heldout"] = {**out.get("heldout", {}), **pool(held)}
    if tracked and held:
        t, h = out["tracked"], out["heldout"]
        gap = {k: t[k] - h[k] if t[k] is not None and h[k] is not None else None for k in ("proven_pct", "possibly_pct", "known_facts_pct")}
        gap.update({k: t[k] / h[k] if t[k] and h[k] else None for k in ("wall_geomean_s", "rss_geomean_mb")})
        out["combined"] = {**pool({**tracked, **held}), "gap": gap}
    out["wrong"] = wrong_claims(e)
    sp = e.get("sitepackages")
    if sp and "error" not in sp:
        fol = [m.get("follow", {}) for m in sp.values()]
        out["sitepackages"] = {"gained": sum((r.get("vs_none") or {}).get("gained", 0) for r in fol),
                               "lost": sum((r.get("vs_none") or {}).get("lost", 0) for r in fol),
                               "finished": sum(1 for r in fol if r.get("finished")),
                               "follow_wall_s": sum(r.get("wall_s", 0) for r in fol)}
    return out


def wrong_claims(e: dict) -> dict:
    """The places where a release's claims miss real Python, pooled: fuzz unsound checks at the
    fresh seeds, held-out `sound` sites not in the baseline, each protocol enumerator's missed
    points (`parts`; None: not measured, in `missing`); `total` the sum of those measured (a
    lower bound where any is missing; None where none is)."""
    f, hs, pr = e.get("fuzz") or {}, e.get("heldsound") or {}, e.get("protocols") or {}
    parts = {"fuzz": f.get("unsound") if f.get("checks") else None, "held-out": hs.get("new") if "error" not in hs else None}
    parts.update({f"protocol {k}": (pr.get(k) or {}).get("missed") for k in PROTOCOLS})
    have = [v for v in parts.values() if v is not None]
    return {"parts": parts, "total": sum(have) if have else None, "missing": [k for k, v in parts.items() if v is None]}


def tracked_runs(e: dict) -> dict | None:
    """The tracked set's runs (the eight's, the cost set's), or None while either is missing."""
    bench, cs = e.get("bench") or {}, (e.get("costset") or {}).get("runs")
    if len(bench) != len(PROGRAMS) or not cs:
        return None
    return {**{"eight/" + p: r for p, r in bench.items()}, **{"costset/" + m: r for m, r in cs.items()}}


def pool(runs: dict) -> dict:
    """Pooled indices over runs {name: result} (bench/heldout.py's `pooled`): verdict and fact
    shares over all runs that finished, geometric means of wall time (a cut run: its timeout)
    and peak memory (runs that finished), and the cuts."""
    tiers, facts = Counter(), Counter()
    for r in runs.values():
        if r.get("finished"):
            tiers.update(r.get("tiers", {}))
            facts.update(r.get("facts", {}))
    nt, nf = sum(tiers.values()), facts["unknown"] + facts["partial"] + facts["exact"]
    fin = sum(1 for r in runs.values() if r.get("finished"))
    return {"modules": len(runs), "finished": fin, "cuts": len(runs) - fin + sum(1 for r in runs.values() if r.get("finished") and r.get("cut")),
            "proven_pct": 100 * tiers["proven"] / nt if nt else None, "possibly_pct": 100 * tiers["possibly"] / nt if nt else None,
            "known_facts_pct": 100 * (facts["partial"] + facts["exact"]) / nf if nf else None, "verdicts": nt, "facts": nf,
            "wall_geomean_s": geomean([r.get("wall_s") for r in runs.values()]),
            "rss_geomean_mb": geomean([r.get("rss_mb") for r in runs.values() if r.get("finished")])}


# the headline metrics: the tracked set's where both releases compared have it, else the eight's
HEAD = {"proven_pct": ("precision", "proven_pct"), "possibly_pct": ("precision", "possibly_pct"), "known_facts_pct": ("precision", "known_facts_pct"),
        "wall_geomean_s": ("speed", "wall_geomean_s"), "rss_geomean_mb": ("speed", "rss_geomean_mb")}


def head(i: dict, key: str, tracked: bool) -> float | None:
    if tracked:
        return (i.get("tracked") or {}).get(key)
    fam, k = HEAD[key]
    return i[fam][k]


def label(e: dict) -> str:
    prs = ", ".join(f"#{p['n']}" for p in e.get("prs", []))
    return f"{e['tag']}" + (f" ({prs})" if prs else "")


def noise(data: dict) -> dict:
    """Relative spread (max/min - 1) of each program's wall time over the repeats, the worst
    over the releases repeated, and of the wall-time geometric mean."""
    worst, gm, gr = {}, [], []
    for e in data["releases"]:
        reps = e.get("bench_repeats", []) + ([{k: v for k, v in e["bench"].items()}] if e.get("bench") else [])
        if len(reps) < 2:
            continue
        for p in PROGRAMS:
            ws = [r[p]["wall_s"] for r in reps if r.get(p, {}).get("finished")]
            if len(ws) >= 2:
                worst[p] = max(worst.get(p, 0), max(ws) / min(ws) - 1)
        gms = [geomean([r[p]["wall_s"] for p in PROGRAMS if p in r]) for r in reps]
        gms = [x for x in gms if x]
        if len(gms) >= 2:
            gm.append(max(gms) / min(gms) - 1)
        grs = [x for x in (geomean([r[p].get("rss_mb") for p in PROGRAMS if p in r]) for r in reps) if x]
        if len(grs) >= 2:
            gr.append(max(grs) / min(grs) - 1)
    return {"wall_spread": {p: round(v, 4) for p, v in worst.items()}, "geomean_spread": max(gm) if gm else None,
            "rss_geomean_spread": max(gr) if gr else None}


def cmd_report(a) -> None:
    data = load_data()
    rels = data["releases"]
    idx = [indices(e) for e in rels]
    lines = []
    w = lines.append
    nz = noise(data)
    w(f"Measured with {data.get('measure_ref', '?')[:10]}'s tests on {data.get('machine', '?')}; fuzz seeds {data.get('fuzz_seeds')} x {data.get('fuzz_n')} programs per kind.\n")
    w("Proven %, known facts %, wall and RSS geomeans, cuts and finished: the tracked set (the eight + the cost set, 31 runs, "
      "pooled); `↳ eight`: the eight alone (the only numbers of releases measured before the cost set, marked †).\n")
    w("| release | engine / models lines | golden pass | fuzz unsound / checks (crashed) | budget=0 pass | proven % | known facts % | wall geomean | RSS geomean | cuts | finished | site-pkg gained / lost |")
    w("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    fmt = lambda x, d=1: "–" if x is None else f"{x:.{d}f}"

    def headline_cells(t: dict, mark: str = "") -> str:
        return (f"{fmt(t['proven_pct'])}{mark} | {fmt(t['known_facts_pct'])}{mark} | {fmt(t['wall_geomean_s'], 2)} s{mark} | "
                f"{fmt(t['rss_geomean_mb'], 0)} MB{mark} | {fmt(t.get('cuts'), 0)} | {fmt(t['finished'], 0)}/{t['modules']}")

    for e, i in zip(rels, idx):
        g, f, b0 = e.get("golden", {}), e.get("fuzz", {}), e.get("budget0", {})
        sp = i.get("sitepackages")
        eight = {**{k: i[fam][k2] for k, (fam, k2) in HEAD.items()}, "finished": i["speed"]["finished"], "modules": len(PROGRAMS),
                 "cuts": sum(1 for r in (e.get("bench") or {}).values() if not r.get("finished") or r.get("cut")) if e.get("bench") else None}
        main = headline_cells(i["tracked"]) if i.get("tracked") else headline_cells(eight, "†")
        w(f"| {label(e)} | {fmt(i['size']['engine_lines'], 0)} / {fmt(i['size']['model_lines'], 0)} | {g.get('passed', '–')}/{g.get('total', '–')} | "
          f"{f.get('unsound', '–')} / {f.get('checks', '–')} ({f.get('crashed', '–')}) | {b0.get('passed', '–')} | {main} | {f'{sp['gained']} / {sp['lost']}' if sp else '–'} |")
        if i.get("tracked"):
            w(f"| ↳ eight | | | | | {headline_cells(eight)} | |")
    # top iterations per family: the change of the family's headline against the release before
    # (precision, speed, memory: on the tracked set where both releases have it, else the eight)
    def prec(i, t):
        parts = [i["precision"]["budget0_pct"], head(i, "proven_pct", t), head(i, "known_facts_pct", t)]
        return sum(parts) / 3 if None not in parts else None

    neglog = lambda x: -math.log(x) * 100 if x else None
    heads = {
        "soundness (golden cases passing + fuzz unsound per 10k)": lambda i, t: (i["soundness"]["golden_pct"] or 0) - (i["soundness"]["unsound_per_10k"] or 0) / 100 if i["soundness"]["index"] is not None else None,
        "precision (index: budget=0 %, proven %, known facts %)": prec,
        "speed (wall-time geomean, lower is better; negated)": lambda i, t: neglog(head(i, "wall_geomean_s", t)),
        "memory (RSS geomean, lower is better; negated)": lambda i, t: neglog(head(i, "rss_geomean_mb", t)),
        "engine size (lines, fewer is better; negated)": lambda i, t: -i["size"]["engine_lines"] if i["size"]["engine_lines"] else None,
    }
    for name, h in heads.items():
        deltas = []
        for k in range(1, len(rels)):
            t = idx[k - 1].get("tracked") is not None and idx[k].get("tracked") is not None
            x, y = h(idx[k - 1], t), h(idx[k], t)
            if x is not None and y is not None:
                deltas.append((y - x, rels[k]))
        deltas = sorted((t for t in deltas if t[0] > 0), key=lambda t: -t[0])
        w(f"\n**Top 10, {name}**\n")
        w("| release | change | pull requests |")
        w("| --- | --- | --- |")
        for d, e in deltas[:10]:
            w(f"| {e['tag']} | {d:+.2f} | {'; '.join(f'#{p['n']} {p['title']}' for p in e.get('prs', []))} |")
    w("\n" + combined_table(rels, idx, fmt))
    w(f"\nNoise (repeats): {json.dumps(nz)}")
    w("\n" + flattening(rels, idx, {k: (lambda h: lambda i: h(i, False))(h) for k, h in heads.items()}))
    rows: list = []
    w("\n" + stop_check(rels, idx, nz, rows=rows))
    data["stop"] = {"criterion": f"3 consecutive releases with no held-out metric moving beyond noise and 0 unsound fresh-seed fuzz checks", "rows": rows, "noise": nz}
    text = "\n".join(lines)
    save_data(data)
    if a.md:
        Path(a.md).write_text(text + "\n")
    print(text)


def combined_table(rels: list[dict], idx: list[dict], fmt) -> str:
    """The combined headline (tracked + held-out pooled) beside the two parts and their gap."""
    out = ["**Combined headline: tracked (tuned-on: the eight + the cost set) and held-out pooled, all with no deadline; "
           "the gap tuned-on minus held-out (time and memory: their ratio) is the overfitting signal**", "",
           "| release | combined proven % | combined known facts % | combined wall geomean | combined RSS geomean | proven % tuned / held-out (gap) | "
           "known facts % tuned / held-out (gap) | wall tuned / held-out (×) | RSS tuned / held-out (×) | cuts tuned / held-out |",
           "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"]
    for e, i in zip(rels, idx):
        c, t, h = i.get("combined"), i.get("tracked"), i.get("heldout")
        if not h:
            continue
        if not c:
            out.append(f"| {e['tag']} | – | – | – | – | – / {fmt(h['proven_pct'])} | – / {fmt(h['known_facts_pct'])} | – / {fmt(h['wall_geomean_s'], 2)} s | "
                       f"– / {fmt(h['rss_geomean_mb'], 0)} MB | – / {h.get('cuts', '–')} |")
            continue
        g = c["gap"]
        out.append(f"| {e['tag']} | {fmt(c['proven_pct'])} | {fmt(c['known_facts_pct'])} | {fmt(c['wall_geomean_s'], 2)} s | {fmt(c['rss_geomean_mb'], 0)} MB | "
                   f"{fmt(t['proven_pct'])} / {fmt(h['proven_pct'])} ({fmt(g['proven_pct'], 1)}) | {fmt(t['known_facts_pct'])} / {fmt(h['known_facts_pct'])} ({fmt(g['known_facts_pct'], 1)}) | "
                   f"{fmt(t['wall_geomean_s'], 2)} / {fmt(h['wall_geomean_s'], 2)} s ({fmt(g['wall_geomean_s'], 1)}×) | "
                   f"{fmt(t['rss_geomean_mb'], 0)} / {fmt(h['rss_geomean_mb'], 0)} MB ({fmt(g['rss_geomean_mb'], 1)}×) | {t['cuts']} / {h['cuts']} |")
    return "\n".join(out)


def flattening(rels: list[dict], idx: list[dict], heads: dict) -> str:
    """Per headline: where the total change since the first loop release (v0.1.8) was 50% / 90%
    done, and the mean absolute change per release over the last 10 against the first 30."""
    out = ["| headline | total change | 50% reached | 90% reached | mean abs. change per release: first 30 / last 10 |", "| --- | --- | --- | --- | --- |"]
    start = next((k for k, r in enumerate(rels) if r["tag"] == "v0.1.8"), 0)
    for name, h in heads.items():
        ys = [(rels[k]["tag"], h(idx[k])) for k in range(start, len(rels))]
        ys = [(t, y) for t, y in ys if y is not None]
        if len(ys) < 3:
            continue
        total = ys[-1][1] - ys[0][1]
        at = lambda f: next((t for t, y in ys if total and (y - ys[0][1]) / total >= f), "–")
        d = [abs(ys[k][1] - ys[k - 1][1]) for k in range(1, len(ys))]
        out.append(f"| {name.split(' (')[0]} | {total:+.2f} | {at(0.5)} | {at(0.9)} | {sum(d[:30]) / max(len(d[:30]), 1):.2f} / {sum(d[-10:]) / max(len(d[-10:]), 1):.2f} |")
    return "\n".join(out)


# held-out metrics (no iteration tunes against them) and what counts as moving
# held-out metrics (no iteration tunes against them) and what counts as moving; proven %, known
# facts %, time and memory on the tracked set where both releases compared have it, else the eight
HELD_OUT = {
    "fuzz unsound (fresh seeds)": (lambda i, t: i["soundness"]["fuzz_unsound"], 0),
    "budget=0 golden passing": (lambda i, t: i["precision"]["budget0_passed"], 3),
    "proven verdicts %": (lambda i, t: head(i, "proven_pct", t), 0.2),
    "known hover facts %": (lambda i, t: head(i, "known_facts_pct", t), 0.5),
    "wall-time geomean (s)": (lambda i, t: head(i, "wall_geomean_s", t), None),
    "RSS geomean (MB)": (lambda i, t: head(i, "rss_geomean_mb", t), None),
}


def stop_check(rels: list[dict], idx: list[dict], nz: dict, k: int = 3, rows: list | None = None) -> str:
    """The stop criterion: stop after K consecutive releases in which no held-out metric moved
    beyond its noise threshold and fresh-seed fuzzing found nothing unsound. Prints, for the
    last releases, which metrics moved."""
    out = [f"Stop criterion: {k} consecutive releases with no held-out metric moving beyond noise and 0 unsound fresh-seed fuzz checks.", "",
           "| release | " + " | ".join(HELD_OUT) + " | quiet? |", "| --- |" + " --- |" * (len(HELD_OUT) + 1)]
    quiet_run = 0
    for j in range(1, len(rels)):
        cells, quiet = [], True
        for name, (f, thr) in HELD_OUT.items():
            t = idx[j - 1].get("tracked") is not None and idx[j].get("tracked") is not None
            a, b = f(idx[j - 1], t), f(idx[j], t)
            if b is None or a is None:
                cells.append("–")
                quiet = False  # (not measured: not known to be quiet)
                continue
            if name.startswith("fuzz"):
                moved = b > 0 or a != b
                cells.append(f"{b}")
            elif thr is None:
                # (twice the largest spread of repeated runs, at least 2%)
                spread = nz.get("rss_geomean_spread" if "RSS" in name else "geomean_spread") or 0.05
                moved = abs(b / a - 1) > max(2 * spread, 0.02) if a else False
                cells.append(f"{100 * (b / a - 1):+.1f}%" + ("*" if moved else ""))
            else:
                moved = abs(b - a) > thr
                cells.append(f"{b - a:+.2f}" + ("*" if moved else ""))
            quiet &= not moved
        quiet_run = quiet_run + 1 if quiet else 0
        if rows is not None:
            rows.append({"tag": rels[j]["tag"], "cells": dict(zip(HELD_OUT, cells)), "quiet": quiet, "in_a_row": quiet_run})
        if j >= len(rels) - 12:
            out.append(f"| {rels[j]['tag']} | " + " | ".join(cells) + f" | {'yes' if quiet else 'no'} ({quiet_run} in a row) |")
    out.append(f"\n(* moved beyond noise: time and memory, twice the repeats' largest geomean spread ({100 * (nz.get('geomean_spread') or 0):.1f}% / "
               f"{100 * (nz.get('rss_geomean_spread') or 0):.1f}%), at least 2%; counts and shares, the thresholds in HELD_OUT)")
    return "\n".join(out)


def main() -> None:
    global DATA
    if len(sys.argv) > 2 and sys.argv[1] == "_harness":
        sys.exit(harness_main(sys.argv[2], sys.argv[3:]))
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("run", "repeat", "report"):
        p = sub.add_parser(name)
        p.add_argument("--cache", default=str(HERE / "work" / "scoreboard"))
        p.add_argument("--data", default=str(DATA), help="the results file (default: bench/scoreboard.json)")
        p.add_argument("--tags")
        p.add_argument("--ref", action="append")
        if name == "run":
            p.add_argument("--since")
            p.add_argument("--families")
            p.add_argument("--redo")
            p.add_argument("-j", type=int, default=3)
        if name == "repeat":
            p.add_argument("-n", type=int, default=3)
        if name == "report":
            p.add_argument("--md")
    a = ap.parse_args()
    DATA = Path(a.data).resolve()
    {"run": cmd_run, "repeat": cmd_repeat, "report": cmd_report}[a.cmd](a)


if __name__ == "__main__":
    main()
