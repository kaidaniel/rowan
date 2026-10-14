"""Runtime oracle for the torch cases: runs each case under real PyTorch and checks its
hand-written expectations against what actually happens (never against rowan).

    uv run --no-project --with torch --with numpy python tests/torch_oracle.py [-v] [case…]

Every case is a runnable program (a driver under `if __name__ == "__main__":` calls the
functions with concrete inputs). After a line with a directive runs, in that frame:

* `#@shape NAME: [d, *R, …] where …`: NAME's `.shape` (a tensor, or a `torch.Size` / tuple of
  ints) must match, each dim expression evaluated in the frame (`x.shape[0]`, `self.n_embd`,
  `d_model` — `__init__` arguments of `self` are visible too); `?` / `_` dims and `*` rests
  are not checked; `none` means not a tensor; `[…] if C, …` is checked only when C holds;
* `#@dim NAME: EXPR`: NAME == EXPR;
* `#@requires C; …`: each constraint holds;
* `#@shape-error`: an exception is raised at that line (the driver catches it).

`@L<n>` restricts a check to runs called (transitively) from line n.  Every directive line
must run at least once.
"""
from __future__ import annotations

import glob
import math
import os
import re
import runpy
import sys
import traceback
from typing import Any, Dict, List, Optional, Tuple

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from shapes import _split_top, directive_lines, parse_where, split_if  # noqa: E402

import numpy as np  # noqa: E402
import torch  # noqa: E402

torch.manual_seed(0)


class Check:
    def __init__(self, line: int, kind: str, at: Optional[int], name: str, body: str, where: Dict[str, str]):
        self.line, self.kind, self.at, self.name, self.body, self.where = line, kind, at, name, body, where
        self.ran = False
        self.file = ""
        self.fail: Optional[str] = None


def parse(src: str) -> List[Check]:
    out: List[Check] = []
    eff = directive_lines(src)
    for i0, line in enumerate(src.splitlines(), 1):
        i = eff[i0]
        if "#@todo" in line:
            continue
        m = re.search(r"#@(shape|dim)(?:@L(\d+))?\s+(.*)$", line)
        if m and not line[m.start():].startswith("#@shape-error"):
            for clause in _split_top(m.group(3), ";"):
                cm = re.match(r"\s*([\w.]+)\s*:\s*(.*)$", clause)
                body, where = parse_where(cm.group(2))
                out.append(Check(i, m.group(1), int(m.group(2)) if m.group(2) else None, cm.group(1), body, where))
            continue
        m = re.search(r"#@requires\s+(.*)$", line)
        if m:
            body, where = parse_where(m.group(1))
            if body.strip() != "none":
                for c in _split_top(body, ";"):
                    out.append(Check(i, "requires", None, "", c, where))
            continue
        m = re.search(r"#@shape-error\b", line)
        if m:
            out.append(Check(i, "error", None, "", "", {}))
    return out


def env_of(frame, inits: Dict[int, Dict[str, Any]], where: Dict[str, str]) -> Dict[str, Any]:
    env: Dict[str, Any] = {"torch": torch, "math": math, "broadcast": lambda *a: tuple(torch.broadcast_shapes(*a)), "ceil": math.ceil}
    env.update(frame.f_globals)
    loc = frame.f_locals
    if "self" in loc and id(loc["self"]) in inits:
        env.update(inits[id(loc["self"])])
    env.update(loc)
    for k, v in where.items():
        env[k] = eval(v.replace("?", "None"), env)
    return env


def shape_of(v: Any) -> Optional[Tuple[int, ...]]:
    if isinstance(v, torch.Tensor):
        return tuple(v.shape)
    if isinstance(v, (np.ndarray, np.generic)):
        return tuple(v.shape)
    if isinstance(v, torch.Size):
        return tuple(v)
    if isinstance(v, tuple) and all(isinstance(e, int) for e in v):
        return v
    return None


def check_shape(c: Check, env: Dict[str, Any], v: Any) -> Optional[str]:
    body, assume = split_if(c.body.strip())
    got = shape_of(v)
    if not all(eval(a, env) for a in assume):
        return None  # the claim is conditional on assumptions that do not hold in this run
    if body == "?":
        return None
    if body == "none":
        return None if got is None else f"is a tensor of shape {list(got)}, want not a tensor"
    if got is None:
        return f"{type(v).__name__} is not a tensor"
    items = _split_top(body[1:-1])
    prefix: List[Any] = []
    suffix: List[Any] = []
    rest: Any = None
    has_rest = False
    for it in items:
        if it.startswith("*"):
            has_rest = True
            r = it[1:].strip()
            rest = None if r in ("", "_") else tuple(eval(r, env))
        else:
            val = None if it.strip() in ("?", "_") else eval(it, env)
            (suffix if has_rest else prefix).append(val)
    if not has_rest:
        if len(got) != len(prefix):
            return f"rank {len(got)} {list(got)}, want {body}"
        dims = prefix
    else:
        n_mid = len(got) - len(prefix) - len(suffix)
        if n_mid < 0 or (rest is not None and len(rest) != n_mid):
            return f"shape {list(got)} does not fit {body}"
        dims = prefix + (list(rest) if rest is not None else [None] * n_mid) + suffix
    for g, w in zip(got, dims):
        if w is not None and g != w:
            return f"shape {list(got)}, want {body} = {[d if d is not None else '?' for d in dims]}"
    return None


def case_files(path: str) -> List[str]:
    """The files whose directives a case checks: the entry and, in a package case (a directory
    whose name shares the entry's prefix: `p06_gpt/p06_gpt.py`), every module under it."""
    path = os.path.abspath(path)
    d = os.path.dirname(path)
    prefix = os.path.basename(path).split("_")[0]
    out = [path]
    if prefix and os.path.basename(d).startswith(prefix + "_"):
        out += sorted(os.path.abspath(p) for p in glob.glob(os.path.join(d, "**", "*.py"), recursive=True) if os.path.abspath(p) != path)
    return out


def run_case(path: str, verbose: bool) -> List[str]:
    path = os.path.abspath(path)
    files = case_files(path)
    checks: List[Check] = []
    by_line: Dict[Tuple[str, int], List[Check]] = {}
    for f in files:
        for c in parse(open(f, encoding="utf-8").read()):
            c.file = f
            checks.append(c)
            by_line.setdefault((f, c.line), []).append(c)
    inits: Dict[int, Dict[str, Any]] = {}
    last: Dict[int, int] = {}  # frame id -> last line

    def called_from(frame, line: int) -> bool:
        f = frame.f_back
        while f is not None:
            if f.f_code.co_filename == path and f.f_lineno == line:
                return True
            f = f.f_back
        return False

    def evaluate(frame, line: int) -> None:
        for c in by_line.get((frame.f_code.co_filename, line), []):
            if c.kind == "error":
                continue
            if c.at is not None and not called_from(frame, c.at):
                continue
            c.ran = True
            if c.fail:
                continue
            try:
                env = env_of(frame, inits, c.where)
                if c.kind == "requires":
                    req, conds = split_if(c.body)
                    if all(eval(k, env) for k in conds) and not eval(req, env):
                        c.fail = f"requirement {c.body} does not hold"
                elif c.kind == "dim":
                    if c.body.strip() in ("?", "_"):
                        continue
                    v = env[c.name] if c.name in env else eval(c.name, env)
                    want = eval(c.body, env)
                    if v != want:
                        c.fail = f"{c.name} = {v}, want {c.body} = {want}"
                else:
                    v = env[c.name] if c.name in env else eval(c.name, env)
                    c.fail = check_shape(c, env, v)
            except Exception as e:  # noqa: BLE001
                c.fail = f"cannot evaluate: {type(e).__name__}: {e}"

    def local(frame, event, arg):
        fid = id(frame)
        if event == "line":
            if fid in last and last[fid] != frame.f_lineno:
                evaluate(frame, last[fid])
            last[fid] = frame.f_lineno
        elif event == "return":
            if fid in last:
                evaluate(frame, last.pop(fid))
            if frame.f_code.co_name == "__init__" and "self" in frame.f_locals:
                inits.setdefault(id(frame.f_locals["self"]), {}).update({k: v for k, v in frame.f_locals.items() if k != "self"})
        elif event == "exception":
            for c in by_line.get((frame.f_code.co_filename, frame.f_lineno), []):
                if c.kind == "error":
                    c.ran = True
        return local

    traced = set(files)

    def glob_trace(frame, event, arg):
        if frame.f_code.co_filename in traced:
            return local
        return None

    old_cwd = os.getcwd()
    os.chdir(os.path.dirname(path))
    # (a package case imports its own modules: from its directory, and not another case's
    # modules of the same name)
    sys.path.insert(0, os.path.dirname(path))
    sys.settrace(glob_trace)
    crashed = None
    try:
        runpy.run_path(path, run_name="__main__")
    except BaseException as e:  # noqa: BLE001
        crashed = "".join(traceback.format_exception_only(type(e), e)).strip()
        tb = e.__traceback__
        while tb is not None:
            if tb.tb_frame.f_code.co_filename in traced:
                crashed += f" ({os.path.relpath(tb.tb_frame.f_code.co_filename, os.path.dirname(path))}:{tb.tb_lineno})"
            tb = tb.tb_next
    finally:
        sys.settrace(None)
        os.chdir(old_cwd)
        sys.path.remove(os.path.dirname(path))
        for name, m in list(sys.modules.items()):
            if (getattr(m, "__file__", None) or "").startswith(os.path.dirname(path) + os.sep) or name.split(".")[0] in {os.path.basename(os.path.dirname(f)) for f in files[1:]}:
                del sys.modules[name]
    fails: List[str] = []
    if crashed:
        fails.append(f"program crashed: {crashed}")
    for c in checks:
        label = f"{os.path.relpath(c.file, os.path.dirname(path)) + ':' if c.file != path else ''}L{c.line} {c.kind} {c.name}".rstrip()
        if not c.ran:
            fails.append(f"{label}: never ran" + (f" from L{c.at}" if c.at else "") + (" (no exception raised)" if c.kind == "error" else ""))
        elif c.fail:
            fails.append(f"{label}: {c.fail}")
    if verbose:
        print(f"  {os.path.relpath(path, HERE)}: {len(checks)} checks")
    return fails


def main(argv: List[str]) -> int:
    verbose = "-v" in argv
    args = [a for a in argv if not a.startswith("-")]
    paths = args or sorted(glob.glob(os.path.join(HERE, "torch", "*", "*.py")) + glob.glob(os.path.join(HERE, "numpy", "*", "*.py")))
    # package cases: the entry of a directory below a group (`playground/p06_gpt/p06_gpt.py`)
    if not args:
        for d in sorted(glob.glob(os.path.join(HERE, "torch", "*", "*", "")) + glob.glob(os.path.join(HERE, "numpy", "*", "*", ""))):
            prefix = os.path.basename(os.path.dirname(d)).split("_")[0]
            paths += sorted(glob.glob(os.path.join(d, prefix + "_*.py")))
    bad = 0
    total = 0
    for p in paths:
        fails = run_case(p, verbose)
        total += 1
        if fails:
            bad += 1
            print(f"FAIL {os.path.relpath(p, HERE)}")
            for f in fails:
                print(f"     - {f}")
    print(f"oracle: {total - bad}/{total} cases agree with PyTorch")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
