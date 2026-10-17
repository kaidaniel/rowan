"""On demand vs the whole program, on the golden cases.

Usage: python tests/ondemand_check.py [-v] [-j N] [case…]   (N cases at a time, default 4)

For each case it runs the whole program (`rowan CASE --facts`) and, in one `rowan serve`, a
hover in every function of the case (`{"cmd":"hover"}`: the function run alone for any call,
from the state the module ended in). The values for any call must agree: both come from the
function's general run, so a difference is the other runs' budget spent before it, or a
general instance made otherwise. Verdicts may differ (for any call vs for the program's
calls), but a `missing` on demand where every call of the whole program reads the key
without fail (`proven`, `likely`) would be a false ✗ (a key the module's own code deletes at
its end is missing for any later call): those are listed and make the exit status 1.

Prints the counts: values alike / different / only on demand / only in the whole program.
"""

import ast
import glob
import json
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
BIN = os.environ.get("ROWAN_BIN", os.path.join(ROOT, "rowan", "target", "release", "rowan"))
# `missing` on demand that hold: the module's own code deletes the key at its end (a call
# after it fails), while the program's calls before the delete read it
DELETED_AT_END = {("tests/stdlib/small/s049_os_environ.py", 6), ("tests/pydantic/models/ym03_config_settings.py", 66)}
# left out unless named: `--facts` on it takes ~95 s (rendering the facts of 40^4 nested
# tuples; the analysis itself is done at once), a hover as long
SLOW = {"tests/cases/z638_hashing_wide_nested_tuples.py"}
SUITES = ("cases", "torch", "pandas", "pydantic", "fastapi", "stdlib", "apps", "numpy")


def function_lines(src: str) -> list[int]:
    """A line inside each function of the module (module-level defs, methods of module-level
    classes)."""
    try:
        tree = ast.parse(src)
    except SyntaxError:
        return []
    out = []
    for s in tree.body:
        defs = [s] if isinstance(s, (ast.FunctionDef, ast.AsyncFunctionDef)) else []
        if isinstance(s, ast.ClassDef):
            defs = [m for m in s.body if isinstance(m, (ast.FunctionDef, ast.AsyncFunctionDef))]
        out += [d.body[0].lineno for d in defs]
    return out


def check(path: str, verbose: bool) -> tuple[dict, list[str], list[str]]:
    """One case: (counts, contradicting `missing`s, verbose lines)."""
    counts = {"alike": 0, "different": 0, "only on demand": 0, "only whole": 0, "functions": 0, "failed": 0}
    false_missing, out = [], []
    src = open(path).read()
    lines = function_lines(src)
    if not lines:
        return counts, false_missing, out
    env = dict(os.environ)
    w = subprocess.run([BIN, path, "--facts"], capture_output=True, text=True, env=env, timeout=120)
    if w.returncode != 0:
        return counts, false_missing, out
    whole = json.loads(w.stdout)
    wfacts = {(f["line"], f["name"]): f for f in whole["facts"] if f.get("general")}
    wtiers = {}
    for v in whole["verdicts"]:
        if not v.get("file"):
            wtiers.setdefault((v["line"], v["col"], str(v.get("key_str"))), set()).add(v["tier"])
    p = subprocess.Popen([BIN, "serve"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env)
    spans = []
    for i, line in enumerate(lines):
        if any(a <= line <= b for a, b in spans):
            continue
        p.stdin.write(json.dumps({"id": i, "cmd": "hover", "path": path, "line": line}) + "\n")
        p.stdin.flush()
        r = json.loads(p.stdout.readline())
        if not r.get("ok"):
            counts["failed"] += 1
            if verbose:
                out.append(f"{path}:{line}: {r.get('error')}")
            continue
        counts["functions"] += 1
        a, b = r["span"]
        spans.append((a, b))
        mine = {(f["line"], f["name"]): f for f in r["result"]["facts"]}
        for k, f in mine.items():
            g = wfacts.get(k)
            if g is None:
                counts["only on demand"] += 1
            elif g["text"] == f["text"]:
                counts["alike"] += 1
            else:
                counts["different"] += 1
                if verbose:
                    out.append(f"{os.path.relpath(path, ROOT)}:{k[0]} {k[1]}: whole {g['text'][:100]!r} / on demand {f['text'][:100]!r}")
        counts["only whole"] += sum(1 for k in wfacts if a <= k[0] <= b and k not in mine)
        for v in r["result"]["verdicts"]:
            # (a key the whole program's calls read without fail on every one of them)
            tiers = wtiers.get((v["line"], v["col"], str(v.get("key_str"))), set())
            if v["tier"] == "missing" and tiers and tiers <= {"proven", "likely"} and (os.path.relpath(path, ROOT), v["line"]) not in DELETED_AT_END:
                false_missing.append(f"{os.path.relpath(path, ROOT)}:{v['line']} {v.get('text')}: missing on demand, proven in the whole program")
    p.stdin.write(json.dumps({"id": -1, "cmd": "shutdown"}) + "\n")
    p.stdin.flush()
    p.wait()
    return counts, false_missing, out


def main(argv: list[str]) -> int:
    verbose = "-v" in argv
    jobs = int(argv[argv.index("-j") + 1]) if "-j" in argv else 4
    paths = [a for i, a in enumerate(argv) if not a.startswith("-") and not (i and argv[i - 1] == "-j")]
    if not paths:
        for s in SUITES:
            paths += sorted(glob.glob(os.path.join(HERE, s, "*.py")) + glob.glob(os.path.join(HERE, s, "*", "*.py")))
        paths = [p for p in paths if os.path.relpath(p, ROOT) not in SLOW]
    counts = {"alike": 0, "different": 0, "only on demand": 0, "only whole": 0, "functions": 0, "failed": 0}
    false_missing = []
    # (threads: each case is two rowan processes; results are printed in the cases' order)
    with ThreadPoolExecutor(jobs) as ex:
        for c, fm, out in ex.map(lambda path: check(path, verbose), paths):
            for k in counts:
                counts[k] += c[k]
            false_missing += fm
            for l in out:
                print(l)
    for m in false_missing:
        print(m)
    print(json.dumps(counts), f"{len(false_missing)} contradicting missing")
    return 1 if false_missing else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
