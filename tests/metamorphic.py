"""Metamorphic checks: rowan against itself, no Python model needed. Over the golden suites'
single-file cases, every simple assignment `name = …` on a line of its own gets `#@general name`
(rowan's claim there for the main run and for any call), and the program is changed in ways that
keep its meaning; the changed program's claims must cover the original's at the same places:

  budget0  ROWAN_LIMITS=budget=0 (every call past its budget): less is known, so its claims
           must hold what the full analysis claims
  rename   every local and global variable renamed (`x` -> `x_m`; not parameters, class
           attributes, imported names or keywords): names mean nothing
  wrap     the module code after the last `def` / `class` run as a function's body (`def
           _m(): …` then `_m()`), where no function reads the names it binds
  swap     two adjacent module-level assignments that share no names and call nothing,
           swapped
  general  in the original: the claim for any call of a function covers its main run's claim
           at the same line

A place where the changed program claims less (a value the original claims it excludes, or the
place unreached) is a *miss*: one of the two runs is unsound or the analysis depends on what
should not matter; the oracle (tests/oracle.py) tells which.

    uv run --no-project python tests/metamorphic.py [--checks budget0,rename,wrap,swap,general]
        [--suites cases,stdlib] [-j 3] [-v] [--limit N] [--quick] [case…]
"""
from __future__ import annotations

import argparse
import ast
import io
import json
import keyword
import os
import re
import subprocess
import sys
import tempfile
import time
import tokenize
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUST_BIN = os.environ.get("ROWAN_BIN", str(ROOT / "rowan" / "target" / "release" / "rowan"))
GUARD = ROOT / ".guard" / "run"
CHECKS = ["budget0", "rename", "wrap", "swap", "general"]


def cases(suites: list, only: list) -> list:
    out = []
    for s in suites:
        for p in sorted((ROOT / "tests" / s).glob("*.py")):
            src = p.read_text()
            # (single-file cases run as they are: no argv, no package beside them)
            if "#@argv" in src or any(d.is_dir() and d.name.startswith(p.name.split("_")[0] + "_") for d in p.parent.iterdir()):
                continue
            if only and not any(o in p.name for o in only):
                continue
            out.append(p)
    return out


def assignments(tree: ast.Module) -> dict:
    """{line: name} of the single-line assignments `name = …` (any depth, not in class bodies)."""
    out: dict = {}

    def walk(body, in_class=False):
        for st in body:
            if isinstance(st, ast.Assign) and not in_class and len(st.targets) == 1 and isinstance(st.targets[0], ast.Name) and st.lineno == st.end_lineno:
                out[st.lineno] = st.targets[0].id
            for f in ("body", "orelse", "finalbody", "handlers"):
                sub = getattr(st, f, None)
                if isinstance(sub, list):
                    walk(sub, isinstance(st, ast.ClassDef))
            if isinstance(st, ast.Try):
                for h in st.handlers:
                    walk(h.body)
            if isinstance(st, ast.Match):
                for c in st.cases:
                    walk(c.body)

    walk(tree.body)
    return out


def instrument(lines: list, points: dict) -> list:
    out = list(lines)
    for ln, name in points.items():
        if "#" not in out[ln - 1] and not out[ln - 1].rstrip().endswith("\\"):
            out[ln - 1] = out[ln - 1].rstrip("\n") + f"  #@general {name}\n"
    return out


def run(text: str, limits: str | None) -> dict | None:
    """{(line, name, general): (lits, unknown)} of rowan's rows on the main file."""
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "case.py"
        p.write_text(text)
        cmd = [RUST_BIN, str(p), "--golden"]
        if sys.platform == "darwin" and GUARD.exists():
            cmd = [str(GUARD), "-t", "120", *cmd]
        env = dict(os.environ)
        if limits:
            env["ROWAN_LIMITS"] = limits
        r = subprocess.run(cmd, capture_output=True, text=True, stdin=subprocess.DEVNULL, env=env, cwd=d)
    try:
        doc = json.loads(r.stdout)
    except json.JSONDecodeError:
        return None
    out: dict = {}
    for rc in doc["rows"]:
        if rc.get("file", ""):
            continue
        v = rc["value"]
        k = (rc["line"], rc["name"], bool(rc.get("general")))
        lits, unk = out.get(k, (set(), False))
        out[k] = (lits | set(v["lits"]), unk or v["unknown"] or bool(v.get("objects")) or v.get("untyped", False))
    return out


def covers(a, b) -> bool:
    """Does claim `a` hold every value claim `b` holds?"""
    if a is None:
        return b is None
    if b is None or a[1]:
        return True
    return not b[1] and b[0] <= a[0]


# ---------------------------------------------------------------- the changes
def user_names(tree: ast.Module) -> set:
    """Variables the program binds by assignment, loops, with, except, comprehensions (not
    parameters, functions, classes, imports, class attributes, dunders)."""
    bound, unsafe = set(), set()
    for node in ast.walk(tree):
        if isinstance(node, ast.ClassDef):
            for st in node.body:
                for t in ast.walk(st):
                    if isinstance(t, ast.Name) and isinstance(t.ctx, ast.Store):
                        unsafe.add(t.id)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda)):
            for a in node.args.args + node.args.kwonlyargs + node.args.posonlyargs + [x for x in (node.args.vararg, node.args.kwarg) if x]:
                unsafe.add(a.arg)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            unsafe.add(node.name)
        if isinstance(node, (ast.Import, ast.ImportFrom)):
            for a in node.names:
                unsafe.add((a.asname or a.name).split(".")[0])
        if isinstance(node, ast.keyword) and node.arg:
            unsafe.add(node.arg)
        if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Store):
            bound.add(node.id)
        if isinstance(node, ast.ExceptHandler) and node.name:
            bound.add(node.name)
        if isinstance(node, (ast.Global, ast.Nonlocal)):
            bound.update(node.names)
        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            # (a name the program also spells as text: getattr / globals() may read it)
            unsafe.add(node.value)
    return {n for n in bound - unsafe if not (n.startswith("__") and n.endswith("__")) and not keyword.iskeyword(n)}


def rename(src: str, tree: ast.Module):
    names = user_names(tree)
    if not names or re.search(r"\b(globals|locals|vars|exec|eval)\s*\(", src):
        return None
    toks = list(tokenize.generate_tokens(io.StringIO(src).readline))
    out, prev = [], None
    for t in toks:
        if t.type == tokenize.NAME and t.string in names and not (prev and prev.type == tokenize.OP and prev.string == "."):
            t = t._replace(string=t.string + "_m")
        out.append(t)
        if t.type not in (tokenize.NL, tokenize.COMMENT):
            prev = t
    text = tokenize.untokenize(out)
    if text.count("\n") != src.count("\n"):
        return None
    return text, (lambda ln: ln), (lambda n: n + "_m" if n in names else n)


def wrap(src: str, tree: ast.Module):
    body = tree.body
    last = max((i for i, st in enumerate(body) if isinstance(st, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef, ast.Import, ast.ImportFrom))), default=-1)
    tail = body[last + 1:]
    if not tail or any(isinstance(st, (ast.Global, ast.Nonlocal, ast.Return)) for st in tail) or "__name__" in src or "#@" in "".join(src.splitlines(True)[:1]):
        return None
    bound = {n.id for st in tail for n in ast.walk(st) if isinstance(n, ast.Name) and isinstance(n.ctx, ast.Store)}
    used_in_defs = {n.id for st in body[: last + 1] for n in ast.walk(st) if isinstance(n, ast.Name)}
    if bound & used_in_defs or any(isinstance(n, (ast.Yield, ast.YieldFrom, ast.Await)) for st in tail for n in ast.walk(st)):
        return None
    lines = src.splitlines(True)
    start = tail[0].lineno
    # (a decorator or comment block before the first statement stays where it is)
    new = lines[: start - 1] + ["def _m():\n"] + ["    " + ln if ln.strip() else ln for ln in lines[start - 1:]] + ["\n", "_m()\n"]
    return "".join(new), (lambda ln: ln + 1 if ln >= start else ln), (lambda n: n)


def pure(st: ast.stmt) -> bool:
    return isinstance(st, ast.Assign) and st.lineno == st.end_lineno and not any(isinstance(n, (ast.Call, ast.Await, ast.Yield, ast.NamedExpr, ast.Subscript, ast.Attribute, ast.Lambda, ast.ListComp, ast.DictComp, ast.SetComp, ast.GeneratorExp)) for n in ast.walk(st))


def names_of(st: ast.stmt) -> set:
    return {n.id for n in ast.walk(st) if isinstance(n, ast.Name)}


def swap(src: str, tree: ast.Module):
    body = tree.body
    for a, b in zip(body, body[1:]):
        if pure(a) and pure(b) and b.lineno == a.lineno + 1 and not (names_of(a) & names_of(b)):
            lines = src.splitlines(True)
            i, j = a.lineno - 1, b.lineno - 1
            # (a directive line below either would move with it: not here)
            if any(ln.lstrip().startswith("#@") for ln in lines[j + 1: j + 2]):
                continue
            lines[i], lines[j] = lines[j], lines[i]
            la, lb = a.lineno, b.lineno
            return "".join(lines), (lambda ln: lb if ln == la else la if ln == lb else ln), (lambda n: n)
    return None


# ---------------------------------------------------------------- one case
def check_case(path: Path, checks: list) -> dict:
    src = path.read_text()
    m = re.search(r"^#@limits\s+(\S+)\s*$", src, re.M)
    limits = m.group(1) if m else None
    try:
        tree = ast.parse(src)
    except SyntaxError:
        return {"case": path.name, "skipped": "syntax"}
    points = assignments(tree)
    if not points:
        return {"case": path.name, "skipped": "no assignments"}
    lines = src.splitlines(True)
    base = run("".join(instrument(lines, points)), limits)
    if base is None:
        return {"case": path.name, "skipped": "rowan failed"}
    res = {"case": path.name, "points": len(points), "checked": Counter(), "misses": {}}

    def compare(name, other, lmap, nmap):
        miss = []
        for (ln, nm, gen), claim in base.items():
            if ln not in points or points[ln] != nm:
                continue
            res["checked"][name] += 1
            got = other.get((lmap(ln), nmap(nm), gen))
            if got is None and gen:
                continue
            if not covers(got, claim):
                miss.append(f"L{ln} {nm}{' (any call)' if gen else ''}: {fmt(claim)} -> {fmt(got)}")
        if miss:
            res["misses"][name] = miss

    for c in checks:
        if c == "general":
            miss = []
            for (ln, nm, gen), claim in base.items():
                if gen or ln not in points:
                    continue
                g = base.get((ln, nm, True))
                if g is None:
                    continue
                res["checked"]["general"] += 1
                if not covers(g, claim):
                    miss.append(f"L{ln} {nm}: main {fmt(claim)}, any call {fmt(g)}")
            if miss:
                res["misses"]["general"] = miss
            continue
        if c == "budget0":
            other = run("".join(instrument(lines, points)), ",".join(x for x in (limits, "budget=0") if x))
            if other is not None:
                compare(c, other, lambda ln: ln, lambda n: n)
            continue
        made = {"rename": rename, "wrap": wrap, "swap": swap}[c](src, tree)
        if made is None:
            continue
        text, lmap, nmap = made
        try:
            t2 = ast.parse(text)
        except SyntaxError:
            continue
        pts2 = assignments(t2)
        other = run("".join(instrument(text.splitlines(True), pts2)), limits)
        if other is not None:
            compare(c, other, lmap, nmap)
    return res


def fmt(c) -> str:
    if c is None:
        return "unreached"
    lits, unk = c
    return "{" + ", ".join(sorted(lits)) + "}" + (" | ?" if unk else "")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("case", nargs="*")
    ap.add_argument("--checks", default=",".join(CHECKS))
    ap.add_argument("--suites", default="cases,stdlib")
    ap.add_argument("-j", type=int, default=3)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--quick", action="store_true", help="the CI bound: every 30th case")
    ap.add_argument("-v", action="store_true")
    a = ap.parse_args()
    checks = a.checks.split(",")
    paths = cases(a.suites.split(","), a.case)
    if a.quick:
        paths = paths[::30]
    if a.limit:
        paths = paths[: a.limit]
    t0 = time.time()
    with ThreadPoolExecutor(a.j) as ex:
        results = list(ex.map(lambda p: check_case(p, checks), paths))
    checked, missed, cases_missed = Counter(), Counter(), Counter()
    for r in results:
        checked.update(r.get("checked", {}))
        for k, ms in r.get("misses", {}).items():
            missed[k] += len(ms)
            cases_missed[k] += 1
            if a.v or cases_missed[k] <= 3:
                print(f"MISS {k} {r['case']}:\n  " + "\n  ".join(ms[:6]))
    print(f"{len(paths)} cases ({sum(1 for r in results if 'skipped' in r)} skipped), {time.time() - t0:.0f} s")
    for k in checks:
        print(f"  {k:8} {checked[k]:6} places checked, {missed[k]} missed in {cases_missed[k]} cases")
    sys.exit(1 if sum(missed.values()) else 0)


if __name__ == "__main__":
    main()
