"""Runtime oracle for rowan's key-set claims: runs a case under real Python (and pandas) and
checks its hand-written `#@` expectations against what actually happens (never against
rowan).

    uv run --no-project --with pandas python tests/oracle.py [--strict|--lenient] [-v] [case…]

Default: `tests/pandas/*/*.py` strict, `tests/cases/` lenient. After a directive's line runs,
in every run that reaches it (one process per case, scratch working directory, timeout):

* `#@row NAME: …` — the keys K of NAME's value (a DataFrame's columns, a dict's keys, a Series'
  index (an `iterrows` row), a namedtuple's fields, an object's attributes): `definite ⊆ K`,
  `absent ∩ K = ∅`, `K ⊆ present` when `closed`;
* `#@value NAME: exactly={…}` — the value is one of these;
* `#@ ok k` — the line never raised a KeyError / AttributeError for k (also from a call made
  on it); `#@ missing k` — every run of the line raised one; `#@ possibly k` (strict only) —
  some run raised one and some did not;
* `#@schema read_csv@L<n>: always={…} only={…}` (strict only) — reruns with `read_csv` at line n
  returning the frame without one column: dropping an `always` column must raise a KeyError /
  AttributeError for it, dropping a column outside `only` and `sometimes` must not.

Strict: every directive line must run and the program must not crash. Lenient (cases
written before the oracle existed: uncalled functions, missing input files, deliberate
failures that end the run): unreached directives are only counted; a crash ends the run.
"""
from __future__ import annotations

import ast
import glob
import json
import os
import re
import runpy
import subprocess
import sys
import tempfile
from typing import Any, Dict, List, Optional, Set, Tuple

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import run_golden  # noqa: E402  (directive parsing)

CHECKED_TIERS = {"proven", "missing", "possibly"}


def parse(src: str):
    verdict_exp, row_exp, schema_exp = run_golden.collect_expectations(src)
    value_exp = {k: list(v) for k, v in run_golden.VALUE_EXP.items()}
    verdicts = {ln: [(t, k) for t, k in vs if t in CHECKED_TIERS and k is not None] for ln, vs in verdict_exp.items()}
    verdicts = {ln: vs for ln, vs in verdicts.items() if vs}
    return verdicts, row_exp, value_exp, schema_exp


def keys_of(v: Any) -> Optional[Set[Any]]:
    try:
        import pandas as pd  # noqa: F401
        if isinstance(v, pd.DataFrame):
            return set(v.columns)
        if isinstance(v, pd.Series):
            return set(v.index)
    except ImportError:
        pass
    if isinstance(v, dict):
        return set(v.keys())
    if isinstance(v, tuple) and hasattr(v, "_fields"):
        return set(v._fields)
    if hasattr(v, "keys") and callable(getattr(v, "keys")):
        try:
            return set(v.keys())
        except Exception:  # noqa: BLE001
            return None
    if hasattr(v, "__dict__") and not isinstance(v, type) and not callable(v):
        return set(vars(v).keys())
    return None


def row_problem(spec: Dict[str, Any], v: Any) -> Optional[str]:
    k = keys_of(v)
    if k is None:
        return None
    definite = spec.get("definite", set())
    absent = spec.get("absent", set())
    if not definite <= k:
        return f"keys {sorted(map(repr, k))} lack definite {sorted(map(repr, definite - k))}"
    if absent & k:
        return f"keys {sorted(map(repr, k))} hold absent {sorted(map(repr, absent & k))}"
    if spec.get("closed") and "present" in spec and not k <= spec["present"]:
        return f"keys {sorted(map(repr, k))} not within closed present {sorted(map(repr, spec['present']))}"
    return None


def raised_for(e: BaseException, key: Any) -> bool:
    if isinstance(e, KeyError):
        return bool(e.args) and e.args[0] == key
    if isinstance(e, AttributeError):
        return isinstance(key, str) and (getattr(e, "name", None) == key or f"'{key}'" in str(e))
    return False


class Run:
    """One execution of the case (optionally with a read_csv column dropped)."""

    def __init__(self, path: str, verdicts, rows, values, drop: Optional[Tuple[int, Any]] = None):
        self.path, self.verdicts, self.rows, self.values, self.drop = path, verdicts, rows, values, drop
        self.ran: Set[int] = set()  # lines run
        self.line_runs: Dict[int, int] = {}
        self.line_raises: Dict[Tuple[int, Any], Set[int]] = {}  # (line, key) -> ids of exceptions
        self.raised_keys: Set[Any] = set()  # keys of KeyErrors / AttributeErrors raised anywhere
        self.fails: List[str] = []
        self.crash: Optional[str] = None
        self.lines_with = set(verdicts) | set(rows) | set(values)

    def check_after(self, frame, line: int) -> None:
        loc = frame.f_locals
        glb = frame.f_globals
        for name, spec in self.rows.get(line, []):
            if name in loc or name in glb:
                p = row_problem(spec, loc[name] if name in loc else glb[name])
                if p:
                    self.fails.append(f"L{line} row {name}: {p}")
        for name, spec in self.values.get(line, []):
            if "exactly" in spec and (name in loc or name in glb):
                v = loc[name] if name in loc else glb[name]
                try:
                    ok = any(v == w and type(v) is not bool or v is w for w in spec["exactly"])
                except Exception:  # noqa: BLE001
                    ok = True
                if not ok:
                    self.fails.append(f"L{line} value {name}: {v!r}, claimed one of {sorted(map(repr, spec['exactly']))}")

    def go(self) -> None:
        path = self.path
        last: Dict[int, int] = {}
        seen_exc: Dict[int, Set[int]] = {}

        def local(frame, event, arg):
            fid = id(frame)
            if event == "line":
                if fid in last and last[fid] != frame.f_lineno:
                    self.check_after(frame, last[fid])
                ln = frame.f_lineno
                last[fid] = ln
                self.ran.add(ln)
                self.line_runs[ln] = self.line_runs.get(ln, 0) + 1
            elif event == "return":
                if fid in last:
                    self.check_after(frame, last.pop(fid))
            elif event == "exception":
                e = arg[1]
                ln = frame.f_lineno
                for tier, key in self.verdicts.get(ln, []):
                    if raised_for(e, key):
                        self.line_raises.setdefault((ln, key), set()).add(id(e))
                if isinstance(e, KeyError) and e.args:
                    self.raised_keys.add(e.args[0])
                if isinstance(e, AttributeError) and getattr(e, "name", None):
                    self.raised_keys.add(e.name)
                # a line whose statement raised does not run its "after" check
                last.pop(fid, None)
            return local

        def glob_trace(frame, event, arg):
            if frame.f_code.co_filename == path:
                return local
            return None

        # mutation of read_csv (schemas)
        if self.drop is not None:
            import pandas as pd
            real = pd.read_csv
            line, col = self.drop

            def patched(*a, **kw):
                df = real(*a, **kw)
                f = sys._getframe(1)
                if f.f_code.co_filename == path and f.f_lineno == line and col in df.columns:
                    df = df.drop(columns=[col])
                return df

            pd.read_csv = patched
        sys.settrace(glob_trace)
        try:
            runpy.run_path(path, run_name="__main__")
        except SystemExit:
            pass
        except BaseException as e:  # noqa: BLE001
            if isinstance(e, KeyError) and e.args:
                self.raised_keys.add(e.args[0])
            if isinstance(e, AttributeError) and getattr(e, "name", None):
                self.raised_keys.add(e.name)
            self.crash = f"{type(e).__name__}: {str(e)[:200]}"
        finally:
            sys.settrace(None)


def run_one(path: str, strict: bool) -> Dict[str, Any]:
    src = open(path, encoding="utf-8").read()
    verdicts, rows, values, schemas = parse(src)
    path = os.path.abspath(path)
    sys.path.insert(0, os.path.dirname(path))
    r = Run(path, verdicts, rows, values)
    r.go()
    fails = list(r.fails)
    unreached = 0
    total = 0
    for ln, vs in verdicts.items():
        for tier, key in vs:
            total += 1
            runs = r.line_runs.get(ln, 0)
            raises = len(r.line_raises.get((ln, key), set()))
            if runs == 0:
                unreached += 1
                if strict:
                    fails.append(f"L{ln} {tier} {key!r}: never ran")
                continue
            # several tiers for one key on a line (the verdict differs by call): each holds in
            # some run; one tier: in every run
            mixed = len({t for t, k in vs if k == key}) > 1
            if tier == "proven" and raises and (not mixed or raises == runs):
                fails.append(f"L{ln} ok {key!r}: raised for {key!r} ({raises} of {runs} runs)")
            if tier == "missing" and raises < runs and (not mixed or raises == 0):
                fails.append(f"L{ln} missing {key!r}: present in {runs - raises} of {runs} runs")
            if tier == "possibly" and strict and (raises == 0 or raises == runs):
                fails.append(f"L{ln} possibly {key!r}: {'never' if raises == 0 else 'always'} missing in {runs} runs (show both)")
    for ln, specs in list(rows.items()) + list(values.items()):
        for _ in specs:
            total += 1
            if ln not in r.ran:
                unreached += 1
                if strict:
                    fails.append(f"L{ln}: never ran")
    if strict and r.crash:
        fails.append(f"program crashed: {r.crash}")
    # schemas: mutate read_csv
    if strict:
        for label, spec in schemas:
            m = re.match(r"read_csv@L(\d+)", label)
            if not m:
                continue
            line = int(m.group(1))
            total += 1
            for col in spec.get("always", set()):
                rr = run_child(path, ["--drop", str(line), json.dumps(col)])
                if col not in rr.get("raised_keys", []):
                    fails.append(f"schema {label}: dropping always-column {col!r} raised nothing for it")
            if "only" in spec:
                allowed = spec["only"] | spec.get("sometimes", set()) | spec.get("always", set())
                cols = run_child(path, ["--columns", str(line)]).get("columns", [])
                for col in cols:
                    if col in allowed:
                        continue
                    rr = run_child(path, ["--drop", str(line), json.dumps(col)])
                    if col in rr.get("raised_keys", []):
                        fails.append(f"schema {label}: column {col!r} outside only={sorted(allowed)} is required")
    return {"fails": fails, "total": total, "unreached": unreached, "crash": r.crash}


def run_child(path: str, extra: List[str]) -> Dict[str, Any]:
    with tempfile.TemporaryDirectory() as tmp:
        try:
            out = subprocess.run([sys.executable, __file__, "--child", path, *extra], capture_output=True, text=True, timeout=60, cwd=tmp, env=dict(os.environ, PYTHONHASHSEED="0"))
        except subprocess.TimeoutExpired:
            return {"fails": ["timeout"], "total": 0, "unreached": 0, "crash": "timeout"}
        lines = [ln for ln in out.stdout.splitlines() if ln.startswith("@@ORACLE ")]
        if not lines:
            return {"fails": [f"oracle child failed: {out.stderr.strip()[-300:]}"], "total": 0, "unreached": 0, "crash": "child"}
        return json.loads(lines[-1][len("@@ORACLE "):])


def child_main(argv: List[str]) -> None:
    path = argv[0]
    strict = "--strict" in argv
    # the case sees its own directory (data files, sibling modules), works in a scratch dir
    for src in glob.glob(os.path.join(os.path.dirname(os.path.abspath(path)), "*")):
        if os.path.isfile(src) and not src.endswith(".py"):
            try:
                os.symlink(src, os.path.join(os.getcwd(), os.path.basename(src)))
            except OSError:
                pass
    if "--drop" in argv:
        i = argv.index("--drop")
        line, col = int(argv[i + 1]), json.loads(argv[i + 2])
        src = open(path, encoding="utf-8").read()
        verdicts, rows, values, _ = parse(src)
        sys.path.insert(0, os.path.dirname(os.path.abspath(path)))
        r = Run(os.path.abspath(path), {}, {}, {}, drop=(line, col))
        r.go()
        res = {"raised_keys": [k for k in r.raised_keys if isinstance(k, (str, int, float, bool))]}
    elif "--columns" in argv:
        line = int(argv[argv.index("--columns") + 1])
        import pandas as pd
        real = pd.read_csv
        seen: List[str] = []
        apath = os.path.abspath(path)

        def spy(*a, **kw):
            df = real(*a, **kw)
            f = sys._getframe(1)
            if f.f_code.co_filename == apath and f.f_lineno == line:
                seen.extend(c for c in df.columns if c not in seen)
            return df

        pd.read_csv = spy
        sys.path.insert(0, os.path.dirname(apath))
        try:
            runpy.run_path(apath, run_name="__main__")
        except BaseException:  # noqa: BLE001
            pass
        res = {"columns": seen}
    else:
        res = run_one(path, strict)
    sys.stdout.flush()
    print("@@ORACLE " + json.dumps(res, default=repr))


def main(argv: List[str]) -> int:
    if argv and argv[0] == "--child":
        child_main(argv[1:])
        return 0
    verbose = "-v" in argv
    force = "strict" if "--strict" in argv else "lenient" if "--lenient" in argv else None
    args = [a for a in argv if not a.startswith("-")]
    if args:
        paths = args
    else:
        paths = sorted(glob.glob(os.path.join(HERE, "pandas", "*", "*.py"))) + sorted(glob.glob(os.path.join(HERE, "cases", "*.py")))
        # package cases: the entry module (its name shares the directory's prefix)
        for d in sorted(glob.glob(os.path.join(HERE, "cases", "*", ""))):
            prefix = os.path.basename(os.path.dirname(d)).split("_")[0]
            paths += sorted(glob.glob(os.path.join(d, prefix + "_*.py")))
    bad = 0
    checked = unreached = 0
    for p in paths:
        strict = force == "strict" if force else os.sep + "pandas" + os.sep in os.path.abspath(p)
        res = run_child(os.path.abspath(p), ["--strict"] if strict else [])
        checked += res["total"] - res["unreached"]
        unreached += res["unreached"]
        if res["fails"]:
            bad += 1
            print(f"FAIL {os.path.relpath(p, HERE)}{' (strict)' if strict else ''}")
            for f in res["fails"]:
                print(f"     - {f}")
        elif verbose:
            print(f"ok   {os.path.relpath(p, HERE)}: {res['total'] - res['unreached']}/{res['total']} checked" + (f"  (crash: {res['crash']})" if res.get("crash") else ""))
    print(f"oracle: {len(paths) - bad}/{len(paths)} cases agree with Python/pandas; {checked} claims checked, {unreached} not reached")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
