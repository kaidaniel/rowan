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


def _typing_shim() -> None:
    """pydantic >= 2.12 passes `prefer_fwd_module` to `typing._eval_type`, which 3.14.0rc2 (the
    interpreter uv provides here) lacks; drop the argument."""
    import inspect
    import typing
    try:
        params = inspect.signature(typing._eval_type).parameters  # type: ignore[attr-defined]
    except (AttributeError, ValueError):
        return
    if "prefer_fwd_module" not in params:
        real = typing._eval_type  # type: ignore[attr-defined]

        def _eval_type(*a, prefer_fwd_module=False, **kw):  # noqa: ARG001
            return real(*a, **kw)

        typing._eval_type = _eval_type  # type: ignore[attr-defined]


_typing_shim()


def validation_missing(e: BaseException) -> List[Any]:
    """The field names a pydantic ValidationError (or FastAPI's RequestValidationError) reports
    missing (`type == "missing"`, the last element of `loc`)."""
    errs = getattr(e, "errors", None)
    if type(e).__name__ not in ("ValidationError", "RequestValidationError") or not callable(errs):
        return []
    try:
        return [x["loc"][-1] for x in errs() if x.get("type") == "missing" and x.get("loc")]
    except Exception:  # noqa: BLE001
        return []

CHECKED_TIERS = {"proven", "missing", "possibly"}


def parse(src: str):
    verdict_exp, row_exp, schema_exp = run_golden.collect_expectations(src)
    value_exp = {k: list(v) for k, v in run_golden.VALUE_EXP.items()}
    verdicts = {ln: [(t, k) for t, k in vs if t in CHECKED_TIERS and k is not None] for ln, vs in verdict_exp.items()}
    verdicts = {ln: vs for ln, vs in verdicts.items() if vs}
    return verdicts, row_exp, value_exp, schema_exp


def real_pandas():
    """pandas, when the case imported it (`tests/pandas` must not pass for it)."""
    pd = sys.modules.get("pandas")
    return pd if pd is not None and hasattr(pd, "DataFrame") else None


def keys_of(v: Any) -> Optional[Set[Any]]:
    pd = real_pandas()
    if pd is not None:
        if isinstance(v, pd.DataFrame):
            return set(v.columns)
        if isinstance(v, pd.Series):
            return set(v.index)
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
    """Did `e` say that `key` is missing? `KeyError(key)`, AttributeError `.name`, or a message
    naming it (pandas: `Column not found: k`, `"['k'] not in index"`, `Label(s) ['k'] do not
    exist`; numpy: `no field of name k`)."""
    if isinstance(e, KeyError):
        if e.args and e.args[0] == key:
            return True
    if key in validation_missing(e):
        return True
    if isinstance(e, AttributeError) and getattr(e, "name", None) == key:
        return True
    if isinstance(e, (KeyError, AttributeError, ValueError)) and isinstance(key, (str, int)):
        msg = str(e)
        k = re.escape(str(key))
        return bool(re.search(rf"'{k}'|\"{k}\"|Column not found: {k}\b|no field of name {k}\b|has no attribute '?{k}\b", msg))
    return False


def case_files(path: str) -> List[Tuple[str, str]]:
    """(absolute path, source) of the files whose directives a case has: the entry and, in a
    package case (a directory whose name shares the entry's prefix), every module under it."""
    path = os.path.abspath(path)
    with open(path, encoding="utf-8") as f:
        out = [(path, f.read())]
    d = os.path.dirname(path)
    prefix = os.path.basename(path).split("_")[0]
    if prefix and os.path.basename(d).startswith(prefix + "_"):
        for p in sorted(glob.glob(os.path.join(d, "**", "*.py"), recursive=True)):
            if os.path.abspath(p) != path:
                with open(p, encoding="utf-8") as f:
                    out.append((os.path.abspath(p), f.read()))
    return out


class Expect:
    """The directives of all files of a case, keyed by (file, line)."""

    def __init__(self, path: str):
        self.verdicts: Dict[Tuple[str, int], List[Tuple[str, Any]]] = {}
        self.rows: Dict[Tuple[str, int], list] = {}
        self.values: Dict[Tuple[str, int], list] = {}
        self.schemas: List[Tuple[str, str, Dict[str, Any]]] = []  # (file, label, spec)
        self.files = [f for f, _ in case_files(path)]
        for f, src in case_files(path):
            v, r, val, sch = parse(src)
            for ln, x in v.items():
                self.verdicts[(f, ln)] = x
            for ln, x in r.items():
                self.rows[(f, ln)] = x
            for ln, x in val.items():
                self.values[(f, ln)] = x
            for label, spec in sch:
                self.schemas.append((f, label, spec))


# ------------------------------------------------------------------ input mutation (schemas)

SOURCE_FNS = {"read_csv": ("pandas", "read_csv"), "json.loads": ("json", "loads"), "json.load": ("json", "load"),
              "yaml.safe_load": ("yaml", "safe_load"), "yaml.load": ("yaml", "load")}


def parse_source(label: str, file: str) -> Optional[Dict[str, Any]]:
    """`read_csv@L12`, `json.loads@L5['items']['*']`, `param handler.event['body']`."""
    m = re.match(r"^(?P<fn>[\w.]+)@L(?P<line>\d+)(?P<path>(\[[^\]]*\])*)$", label)
    if m and m.group("fn") in SOURCE_FNS:
        return {"kind": "call", "fn": m.group("fn"), "file": file, "line": int(m.group("line")), "path": parse_path(m.group("path"))}
    m = re.match(r"^param (?P<func>[\w.<>]+)\.(?P<arg>\w+)(?P<path>(\[[^\]]*\])*)$", label)
    if m:
        return {"kind": "param", "func": m.group("func"), "arg": m.group("arg"), "path": parse_path(m.group("path"))}
    return None


def parse_path(p: str) -> List[Any]:
    return [ast.literal_eval(x) if x not in ("*", "?") else "*" for x in re.findall(r"\[([^\]]*)\]", p)]


def drop_at(v: Any, path: List[Any], col: Any) -> Any:
    """`v` without key `col` at `path` (copied along the path; `*` = every element / value)."""
    if not path:
        pd = real_pandas()
        if pd is not None and isinstance(v, pd.DataFrame):
            return v.drop(columns=[col]) if col in v.columns else v
        if isinstance(v, dict) and col in v:
            return type(v)((k, x) for k, x in v.items() if k != col) if type(v) is not dict else {k: x for k, x in v.items() if k != col}
        return v
    seg, rest = path[0], path[1:]
    if seg == "*":
        if isinstance(v, list):
            return [drop_at(x, rest, col) for x in v]
        if isinstance(v, dict):
            return {k: drop_at(x, rest, col) for k, x in v.items()}
        return v
    if isinstance(v, dict) and seg in v:
        out = dict(v)
        out[seg] = drop_at(v[seg], rest, col)
        return out
    if isinstance(v, list) and isinstance(seg, int) and -len(v) <= seg < len(v):
        out = list(v)
        out[seg] = drop_at(v[seg], rest, col)
        return out
    return v


def keys_at(v: Any, path: List[Any]) -> List[Any]:
    if not path:
        k = keys_of(v)
        return list(k) if k is not None else []
    seg, rest = path[0], path[1:]
    if seg == "*":
        items = v if isinstance(v, list) else list(v.values()) if isinstance(v, dict) else []
        out: List[Any] = []
        for x in items:
            out += [k for k in keys_at(x, rest) if k not in out]
        return out
    try:
        return keys_at(v[seg], rest)
    except Exception:  # noqa: BLE001
        return []


class Mutator:
    """Applies `drop` (remove key `col` from the source's value) or `spy` (record its keys)
    to one source: a call at a line (read_csv / json.loads …) or a function's parameter."""

    def __init__(self, src: Dict[str, Any], col: Any = None, spy: Optional[List[Any]] = None):
        self.src, self.col, self.spy = src, col, spy

    def apply(self, v: Any) -> Any:
        if self.spy is not None:
            self.spy.extend(k for k in keys_at(v, self.src["path"]) if k not in self.spy)
            return v
        return drop_at(v, self.src["path"], self.col)

    def install(self) -> None:
        src = self.src
        if src["kind"] != "call":
            return
        modname, attr = SOURCE_FNS[src["fn"]]
        try:
            mod = __import__(modname)
        except ImportError:
            return
        real = getattr(mod, attr)
        me = self

        def patched(*a, **kw):
            v = real(*a, **kw)
            f = sys._getframe(1)
            if f.f_code.co_filename == src["file"] and f.f_lineno == src["line"]:
                v = me.apply(v)
            return v

        setattr(mod, attr, patched)

    def on_call(self, frame) -> None:
        src = self.src
        if src["kind"] != "param":
            return
        code = frame.f_code
        if code.co_qualname != src["func"] and code.co_name != src["func"] and not code.co_qualname.endswith("." + src["func"]):
            return
        loc = frame.f_locals
        if src["arg"] in loc:
            new = self.apply(loc[src["arg"]])
            if self.spy is None:
                loc[src["arg"]] = new  # PEP 667: writes through to the fast local


class Run:
    """One execution of the case (optionally with one input source mutated)."""

    def __init__(self, path: str, exp: Expect, mutator: Optional[Mutator] = None):
        self.path, self.exp, self.mutator = path, exp, mutator
        self.files = set(exp.files)
        self.ran: Set[Tuple[str, int]] = set()
        self.line_runs: Dict[Tuple[str, int], int] = {}
        # (file, line, key) -> the exceptions themselves (kept alive: ids of collected ones get reused)
        self.line_raises: Dict[Tuple[str, int, Any], List[BaseException]] = {}
        self.raised_keys: Set[Any] = set()  # keys of KeyErrors / AttributeErrors raised anywhere
        self.fails: List[str] = []
        self.crash: Optional[str] = None

    def note_missing(self, e: BaseException) -> None:
        """The column / key names an exception says are missing (schema mutation checks):
        `KeyError(k)`, AttributeError .name, pydantic's missing fields, and pandas' messages
        naming columns (`Column not found: x`, `"['x'] not in index"`, `Label(s) ['x'] do not
        exist`)."""
        if isinstance(e, KeyError) and e.args and isinstance(e.args[0], (str, int, float, bool, tuple)):
            self.raised_keys.add(e.args[0])
        if isinstance(e, AttributeError) and getattr(e, "name", None):
            self.raised_keys.add(e.name)
        for k in validation_missing(e):
            if isinstance(k, (str, int)):
                self.raised_keys.add(k)
        if isinstance(e, (KeyError, ValueError, AttributeError)):
            msg = str(e)
            for m in re.finditer(r"'([^']+)'", msg):
                self.raised_keys.add(m.group(1))
            m = re.search(r"Column not found: (\S+)", msg)
            if m:
                self.raised_keys.add(m.group(1).strip("'\""))

    def check_after(self, frame, key: Tuple[str, int]) -> None:
        loc = frame.f_locals
        glb = frame.f_globals
        line = key[1]
        where = f"{os.path.basename(key[0])}:" if key[0] != self.path else ""
        for name, spec in self.exp.rows.get(key, []):
            if name in loc or name in glb:
                p = row_problem(spec, loc[name] if name in loc else glb[name])
                if p:
                    self.fails.append(f"{where}L{line} row {name}: {p}")
        for name, spec in self.exp.values.get(key, []):
            if "exactly" in spec and (name in loc or name in glb):
                v = loc[name] if name in loc else glb[name]
                try:
                    ok = any(v == w and type(v) is not bool or v is w for w in spec["exactly"])
                except Exception:  # noqa: BLE001
                    ok = True
                if not ok:
                    self.fails.append(f"{where}L{line} value {name}: {v!r}, claimed one of {sorted(map(repr, spec['exactly']))}")

    def go(self) -> None:
        import threading
        last: Dict[int, Tuple[str, int]] = {}
        files = self.files
        lock = threading.RLock()

        def local(frame, event, arg):
            with lock:
                fid = id(frame)
                fn = frame.f_code.co_filename
                if event == "line":
                    key = (fn, frame.f_lineno)
                    if fid in last and last[fid] != key:
                        self.check_after(frame, last[fid])
                    last[fid] = key
                    self.ran.add(key)
                    self.line_runs[key] = self.line_runs.get(key, 0) + 1
                elif event == "return":
                    if fid in last:
                        self.check_after(frame, last.pop(fid))
                elif event == "exception":
                    e = arg[1]
                    ln = frame.f_lineno
                    for tier, k in self.exp.verdicts.get((fn, ln), []):
                        if raised_for(e, k):
                            seen = self.line_raises.setdefault((fn, ln, k), [])
                            if not any(x is e for x in seen):
                                seen.append(e)
                    self.note_missing(e)
                    # a line whose statement raised does not run its "after" check
                    last.pop(fid, None)
            return local

        def glob_trace(frame, event, arg):
            if frame.f_code.co_filename in files:
                if event == "call" and self.mutator is not None:
                    self.mutator.on_call(frame)
                return local
            return None

        if self.mutator is not None:
            self.mutator.install()
        threading.settrace(glob_trace)
        sys.settrace(glob_trace)
        try:
            runpy.run_path(self.path, run_name="__main__")
        except SystemExit:
            pass
        except BaseException as e:  # noqa: BLE001
            self.note_missing(e)
            self.crash = f"{type(e).__name__}: {str(e)[:200]}"
        finally:
            sys.settrace(None)
            threading.settrace(None)  # type: ignore[arg-type]


def run_one(path: str, strict: bool) -> Dict[str, Any]:
    path = os.path.abspath(path)
    exp = Expect(path)
    sys.path.insert(0, os.path.dirname(path))
    r = Run(path, exp)
    r.go()
    fails = list(r.fails)
    unreached = 0
    total = 0

    def where(f: str) -> str:
        return f"{os.path.relpath(f, os.path.dirname(path))}:" if f != path else ""

    for (f, ln), vs in exp.verdicts.items():
        vs = [(t, k) for t, k in vs]
        for tier, key in vs:
            total += 1
            runs = r.line_runs.get((f, ln), 0)
            raises = len(r.line_raises.get((f, ln, key), []))
            if runs == 0:
                unreached += 1
                if strict:
                    fails.append(f"{where(f)}L{ln} {tier} {key!r}: never ran")
                continue
            # several tiers for one key on a line (the verdict differs by call): each holds in
            # some run; one tier: in every run
            mixed = len({t for t, k in vs if k == key}) > 1
            if tier == "proven" and raises and (not mixed or raises == runs):
                fails.append(f"{where(f)}L{ln} ok {key!r}: raised for {key!r} ({raises} of {runs} runs)")
            if tier == "missing" and raises < runs and (not mixed or raises == 0):
                fails.append(f"{where(f)}L{ln} missing {key!r}: present in {runs - raises} of {runs} runs")
            if tier == "possibly" and strict and (raises == 0 or raises == runs):
                fails.append(f"{where(f)}L{ln} possibly {key!r}: {'never' if raises == 0 else 'always'} missing in {runs} runs (show both)")
    for (f, ln), specs in list(exp.rows.items()) + list(exp.values.items()):
        for _ in specs:
            total += 1
            if (f, ln) not in r.ran:
                unreached += 1
                if strict:
                    fails.append(f"{where(f)}L{ln}: never ran")
    if strict and r.crash:
        fails.append(f"program crashed: {r.crash}")
    # schemas: mutate the source (drop one key) and rerun
    if strict:
        for f, label, spec in exp.schemas:
            src = parse_source(label, f)
            if src is None:
                continue
            total += 1
            enc = json.dumps(src)
            # errors the unmutated program already raises (deliberate misses) say nothing
            baseline = set(run_child(path, ["--drop", enc, json.dumps("\x00none")]).get("raised_keys", []))
            cols = run_child(path, ["--columns", enc]).get("columns", [])
            if not cols:
                fails.append(f"schema {label}: the source never produced keys")
                continue
            for col in spec.get("always", set()):
                if col in baseline:
                    continue
                rr = run_child(path, ["--drop", enc, json.dumps(col)])
                if col not in rr.get("raised_keys", []):
                    fails.append(f"schema {label}: dropping always-key {col!r} raised nothing for it")
            if "only" in spec:
                allowed = spec["only"] | spec.get("sometimes", set()) | spec.get("always", set())
                for col in cols:
                    if col in allowed or col in baseline:
                        continue
                    rr = run_child(path, ["--drop", enc, json.dumps(col)])
                    if col in rr.get("raised_keys", []):
                        fails.append(f"schema {label}: key {col!r} outside only={sorted(allowed)} is required")
    return {"fails": fails, "total": total, "unreached": unreached, "crash": r.crash}


def run_child(path: str, extra: List[str]) -> Dict[str, Any]:
    with tempfile.TemporaryDirectory() as tmp:
        try:
            out = subprocess.run([sys.executable, __file__, "--child", path, *extra], capture_output=True, text=True, timeout=120, cwd=tmp, env=dict(os.environ, PYTHONHASHSEED="0"))
        except subprocess.TimeoutExpired:
            return {"fails": ["timeout"], "total": 0, "unreached": 0, "crash": "timeout"}
        lines = [ln for ln in out.stdout.splitlines() if ln.startswith("@@ORACLE ")]
        if not lines:
            return {"fails": [f"oracle child failed: {out.stderr.strip()[-300:]}"], "total": 0, "unreached": 0, "crash": "child"}
        return json.loads(lines[-1][len("@@ORACLE "):])


def child_main(argv: List[str]) -> None:
    path = os.path.abspath(argv[0])
    strict = "--strict" in argv
    # the case sees its own directory (data files, sibling modules), works in a scratch dir
    for src in glob.glob(os.path.join(os.path.dirname(path), "*")):
        if os.path.isfile(src) and not src.endswith(".py"):
            try:
                os.symlink(src, os.path.join(os.getcwd(), os.path.basename(src)))
            except OSError:
                pass
    sys.path.insert(0, os.path.dirname(path))
    if "--drop" in argv or "--columns" in argv:
        flag = "--drop" if "--drop" in argv else "--columns"
        i = argv.index(flag)
        src = json.loads(argv[i + 1])
        exp = Expect(path)
        seen: List[Any] = []
        mut = Mutator(src, col=json.loads(argv[i + 2])) if flag == "--drop" else Mutator(src, spy=seen)
        exp.verdicts, exp.rows, exp.values, exp.schemas = {}, {}, {}, []
        r = Run(path, exp, mut)
        r.go()
        if flag == "--drop":
            res = {"raised_keys": [k for k in r.raised_keys if isinstance(k, (str, int, float, bool))]}
        else:
            res = {"columns": [k for k in seen if isinstance(k, (str, int, float, bool))]}
    else:
        res = run_one(path, strict)
    sys.stdout.flush()
    print("@@ORACLE " + json.dumps(res, default=repr))


STRICT_SUITES = ("pandas", "pydantic", "fastapi", "stdlib", "apps", "numpy")


def suite_cases() -> List[str]:
    paths: List[str] = []
    for suite in STRICT_SUITES:
        sd = os.path.join(HERE, suite)
        paths += sorted(glob.glob(os.path.join(sd, "*", "*.py")))
        # package cases inside a suite: the entry module of each directory
        for d in sorted(glob.glob(os.path.join(sd, "*", "*", ""))):
            prefix = os.path.basename(os.path.dirname(d)).split("_")[0]
            paths += sorted(glob.glob(os.path.join(d, prefix + "_*.py")))
    paths += sorted(glob.glob(os.path.join(HERE, "cases", "*.py")))
    for d in sorted(glob.glob(os.path.join(HERE, "cases", "*", ""))):
        prefix = os.path.basename(os.path.dirname(d)).split("_")[0]
        paths += sorted(glob.glob(os.path.join(d, prefix + "_*.py")))
    return paths


def main(argv: List[str]) -> int:
    if argv and argv[0] == "--child":
        child_main(argv[1:])
        return 0
    verbose = "-v" in argv
    force = "strict" if "--strict" in argv else "lenient" if "--lenient" in argv else None
    args = [a for a in argv if not a.startswith("-")]
    paths = args or suite_cases()
    bad = 0
    checked = unreached = 0
    for p in paths:
        ap = os.path.abspath(p)
        strict = force == "strict" if force else any(os.sep + s + os.sep in ap for s in STRICT_SUITES)
        res = run_child(ap, ["--strict"] if strict else [])
        checked += res["total"] - res["unreached"]
        unreached += res["unreached"]
        if res["fails"]:
            bad += 1
            print(f"FAIL {os.path.relpath(p, HERE)}{' (strict)' if strict else ''}")
            for f in res["fails"]:
                print(f"     - {f}")
        elif verbose:
            print(f"ok   {os.path.relpath(p, HERE)}: {res['total'] - res['unreached']}/{res['total']} checked" + (f"  (crash: {res['crash']})" if res.get("crash") else ""))
    print(f"oracle: {len(paths) - bad}/{len(paths)} cases agree with Python; {checked} claims checked, {unreached} not reached")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
