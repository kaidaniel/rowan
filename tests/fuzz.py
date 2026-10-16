"""Differential fuzzing of rowan's values against CPython.

Random small programs (lists, dicts and sets changed in place; objects and dicts; calls,
closures, generators, classes; str / number computations) under branches and loops that depend
on `n = len(sys.argv)`. Each program runs under CPython for n = 1..5; every probe (`p3 = ...`)
gets a `#@value p3: includes={...}` directive with the values it took, and the program is
checked as a golden case: rowan must claim nothing that leaves a value out (sound), or say it
does not know. A failing program is shrunk (`--shrink`: statements replaced by `pass` while it
still fails) and written next to it as `NAME_min.py`: a repro to turn into a golden case, with
expectations written by hand.

    uv run --no-project python tests/fuzz.py [KIND…] [--seed S] [-n N] [-j J] [--out DIR] [--shrink]

KIND: lists, objects, calls, scalars (default: all of them). Run it under `.guard/run`;
`FUZZ_TRACE=1` (with `-j 1`) names each program before its check, to find one that a guard killed.
"""
from __future__ import annotations

import argparse
import os
import random
import re
import sys
import tempfile
from concurrent.futures import ProcessPoolExecutor

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from run_golden import run_case  # noqa: E402

COND = ["if n > 2:", "if n % 2 == 0:", "if n == 1:"]
LOOP = ["for _i in range(n):", "for _i in range(n - 1):"]


def block(op, depth: int, count: int, conds=COND) -> list:
    """`count` statements of `op()`, some under a branch or a loop of unknown count."""
    lines: list = []
    for _ in range(count):
        c = random.random()
        if depth < 2 and c < 0.15:
            lines.append(random.choice(conds))
            lines += ["    " + l for l in block(op, depth + 1, random.randrange(1, 4), conds)]
            if random.random() < 0.5:
                lines.append("else:")
                lines += ["    " + l for l in block(op, depth + 1, random.randrange(1, 3), conds)]
        elif depth < 2 and c < 0.25:
            lines.append(random.choice(LOOP))
            lines += ["    " + l for l in block(op, depth + 1, random.randrange(1, 3), conds)]
        else:
            lines += op()
    return lines


# ---------------------------------------------------------------- lists, dicts, sets
LV = ["0", "1", "2", "7", '"s"', '"t"', "None", "1.5"]
LISTS_HEAD = """import sys
def f_app(xs, v):
    xs.append(v)
def f_set(xs, v):
    if xs:
        xs[0] = v
def f_new(xs):
    return xs + [1]
def f_ret(xs, v):
    if v in xs:
        return xs
    return [v]
def pick(x, y):
    return x if n > 2 else y
n = len(sys.argv)
a = [1, 2]
b = ["s"]
c = []
d = {"k": 0}
s = {1}"""


def lists_op() -> list:
    x, y, v, w = random.choice("abc"), random.choice("abc"), random.choice(LV), random.choice(LV)
    return random.choice([
        [f"{x}.append({v})"], [f"{x}.insert({random.randrange(-2, 3)}, {v})"],
        [f"if len({x}) > 1:", f"    {x}[{random.randrange(-1, 2)}] = {v}"], [f"if len({x}) > 1:", f"    del {x}[1]"],
        [f"if {x}:", f"    {x}.pop()"], [f"if {x}:", f"    {x}.pop(0)"], [f"{x}.extend([{v}, {w}])"], [f"{x}.reverse()"],
        [f"{x}[0:1] = [{v}]"], [f"{x} += [{v}]"], [f"{x} = {y}"], [f"{x} = {y}[:]"], [f"{x} = list({y})"],
        [f"{x} = [e for e in {y} if e != {v}]"], [f"{x} = {y} + [{v}]"], [f"{x}.clear()"], [f"{x} = sorted({y}, key=str)"],
        [f"{x}[:] = {y}"], [f"{x} = ({y} * 2)[:99]"], [f"{x} = [{v}, {w}]"], [f"{x}.sort(key=str)"], [f"{x} = list(reversed({y}))"],
        [f"{x}[-1:] = [{v}, {w}]"], [f"{x}, *_r = {y} + [0]", f"{x} = [{x}]"], [f"{x} = [*{y}, {v}]"], [f"f_app({x}, {v})"],
        [f"f_set({x}, {v})"], [f"{x} = f_new({y})"], [f"{x} = f_ret({y}, {v})"], [f"d[{v}] = {x}"], [f"{x} = d.get({v}, {y})"],
        [f"d.setdefault({v}, []).append({w})"], [f"{x} = list(d)"], [f"s.add({v})"], [f"{x} = sorted(s, key=str)"], [f"s.discard({v})"],
        [f"{x} = [e for e in {y} if e in s] + [e for e in s if e not in {y}]"],
        [f"pick({x}, {y}).append({v})"], [f"pick({x}, {y}).insert(0, {v})"], [f"pick({x}, {y}).sort(key=str)"],
        [f"if pick({x}, {y}):", f"    pick({x}, {y})[0] = {v}"], [f"pick({x}, {y}).extend([{v}])"], [f"pick({x}, {y}).clear()"],
        [f"pick(d, {{}})[{v}] = {w}"], [f"pick(s, set()).add({v})"],
    ])


def lists_program():
    probes = ["[e for e in d]", "[e for e in s]", "len(s)"]
    for v in "abc":
        probes += [f"len({v})", f"{v}[0] if {v} else -9", f"{v}[-1] if {v} else -9", f"[e for e in {v}]", f"'s' in {v}", f"7 in {v}"]
    return LISTS_HEAD.split("\n"), block(lists_op, 0, random.randrange(4, 12)), probes


# ---------------------------------------------------------------- objects and dicts
OV = ["0", "1", "2", '"s"', "None", "[1]", "(3,)"]
KEYS = ['"k"', '"m"', '"z"']
OBJECTS_HEAD = """import sys
class Box:
    def __init__(self, v):
        self.v = v
        self.items = []
    def add(self, x):
        self.items.append(x)
        return self
def fill(d, k):
    if k not in d:
        d[k] = 9
    return d[k]
def clean(d):
    return {k: v for k, v in d.items() if v is not None}
def depth(xs, i):
    if i >= len(xs):
        return i
    return depth(xs, i + 1)
def make(v):
    def cb():
        return v
    return cb
def pick(x, y):
    return x if n > 2 else y
n = len(sys.argv)
d = {"k": 0}
e = {}
o = Box(1)
p = Box("s")"""


def objects_op() -> list:
    d, d2, o, o2 = random.choice("de"), random.choice("de"), random.choice("op"), random.choice("op")
    v, k, k2 = random.choice(OV), random.choice(KEYS), random.choice(KEYS)
    return random.choice([
        [f"{d}[{k}] = {v}"], [f"{d}.pop({k}, None)"], [f"if {k} in {d}:", f"    del {d}[{k}]"], [f"{d}.update({{{k}: {v}}})"],
        [f"{d} = dict({d2})"], [f"{d} = {d2}"], [f"{d}.setdefault({k}, {v})"], [f"{d} = {{**{d2}, {k}: {v}}}"],
        [f"{d} = {{kk: vv for kk, vv in {d2}.items() if vv != {v}}}"],
        ["try:", f"    {d}[{k}] = {d2}[{k2}]", "except KeyError:", f"    {d}[{k}] = {v}"],
        [f"{o}.v = {v}"], [f"{o}.add({v})"], [f"if hasattr({o}, 'w'):", f"    del {o}.w"], [f"{o}.w = {o2}.v"], [f"{o} = {o2}"],
        [f"{o} = Box({v})"], [f"{o} = {o2}.add({v})"], [f"{d}[{k}] = {o}.v"], [f"{o}.v = {d}.get({k}, {v})"], [f"{o}.v = fill({d}, {k})"],
        [f"{d} = clean({d2})"], ["j = 0", "while j < n:", "    j += 1", "    if j > 2:", "        break", f"    {d}[{k}] = j"],
        ["try:", f"    {o}.items.pop()", "except IndexError:", f"    {o}.v = {v}"], [f"{o}.v = depth({o}.items, 0)"],
        [f"cb = make({v})", f"{d}[{k}] = cb()"], [f"{o}.items = [x for x in {o}.items if x != {v}]"],
        [f"if isinstance({o}.v, int):", f"    {o}.v += 1"], [f"{o}.v = {o}.items[-1] if {o}.items else {v}"],
        [f"for kk in list({d}):", f"    if kk == {k}:", f"        {d2}[kk] = {v}"], [f"{o}.v = getattr({o2}, 'w', {v})"], [f"{d}.clear()"],
        [f"{o}.items.extend({d}.keys())"], [f"{o}.items.insert(0, {v})"],
        [f"pick({d}, {d2})[{k}] = {v}"], [f"pick({d}, {d2}).pop({k}, None)"], [f"pick({d}, {d2}).update({{{k}: {v}}})"],
        [f"pick({o}, {o2}).v = {v}"], [f"pick({o}, {o2}).add({v})"], [f"pick({d}, {d2}).setdefault({k}, {v})"],
    ])


def objects_program():
    probes = []
    for d in "de":
        probes += [f"{d}.get({k}, -1)" for k in KEYS] + [f"{k} in {d}" for k in KEYS] + [f"len({d})"]
    for o in "op":
        probes += [f"{o}.v", f"getattr({o}, 'w', -1)", f"len({o}.items)", f"{o}.items[0] if {o}.items else -9"]
    return OBJECTS_HEAD.split("\n"), block(objects_op, 0, random.randrange(4, 12)), probes


# ---------------------------------------------------------------- calls, closures, generators, classes
CV = ["0", "1", "2", '"s"', '"t"', "None"]
CALLS_HEAD = '''import sys
G = 0
def setg(v):
    global G
    G = v
def f1(a, b=1):
    if a == b:
        return "eq"
    return a
def f2(*args):
    return args[-1]
def make():
    c = 0
    def inc():
        nonlocal c
        c += 1
        return c
    return inc
cnt = make()
def gen(v):
    yield v
    if v:
        yield "more"
class A:
    k = "A"
    def __init__(self, v):
        self.v = v
    def get(self):
        return self.v
class B(A):
    def get(self):
        return super().get() if self.v else self.k
class C:
    def __init__(self):
        self.val = 0
    def bump(self, v):
        if v:
            self.val = v
obj = C()
def risky(v):
    if v == 1 or v == "s":
        raise ValueError(v)
    return v
def rec(i):
    if i <= 0:
        return "base"
    return rec(i - 1)
def pick(a=None, c=2):
    return a if a is not None else c
cache = {}
class Ctx:
    def __enter__(self):
        self.v = "in"
        return self
    def __exit__(self, *a):
        self.v = "out"
        return False
n = len(sys.argv)
x = 1
y = "s"
z = None'''


def calls_op() -> list:
    x, y, v = random.choice("xyz"), random.choice("xyz"), random.choice(CV)
    return random.choice([
        [f"{x} = f1({y})"], [f"{x} = f1({y}, b={v})"], [f"{x} = f2({y}, {v})"], [f"{x} = f2(*[{y}, {v}])"], [f"{x} = cnt()"],
        [f"{x} = next(gen({y}), {v})"], [f"{x} = [w for w in gen({y})][-1]"], [f"{x} = A({y}).get()"], [f"{x} = B({y}).get()"],
        [f"obj.bump({y})", f"{x} = obj.val"], ["try:", f"    {x} = risky({y})", "except ValueError:", f"    {x} = {v}"],
        ["try:", f"    {x} = risky({y})", "except ValueError:", f"    {x} = {v}", "finally:", f"    {y} = {v}"],
        ["i = 0", "while True:", "    i += 1", "    if i > n:", "        break", "    if i == 2:", "        continue", f"    {x} = i"],
        [f"setg({y})", f"{x} = G"], [f"{x} = rec({random.randrange(4)})"], [f"{x} = pick(**{{'a': {y}}})"], [f"{x} = pick({y}, c={v})"],
        [f"{x} = (lambda q={y}: q)()"], [f"{x} = {y} if {y} else {v}"], [f"{x} = {y} or {v}"], [f"{x} = {y} and {v}"],
        [f"{x} = str({y}) + '!'"], [f"{x} = cache.setdefault({v}, {y})"], [f"{x} = getattr(obj, 'val', {v})"], [f"obj.val = {y}"],
        [f"{x} = f'{{{y}}}-'"], [f"{x} = isinstance({y}, str) and {y}.upper() or {v}"], ["with Ctx() as cm:", f"    {x} = cm.v", f"    {y} = {v}"],
    ])


def calls_program():
    conds = COND + ["if x:", "if y:"]
    return CALLS_HEAD.split("\n"), block(calls_op, 0, random.randrange(4, 12), conds), ["x", "y", "z", "G", "obj.val", "cnt()", "len(cache)"]


# ---------------------------------------------------------------- str and number computations
SV = ['"ab"', '" Ab c "', '"x,y,,z"', '"3"', '"-7"', '""', '"ÄÖ"', '"a.b.c"', '"10"', '"tT"']
NV = ["0", "1", "-3", "7", "2.5", "-0.5", "True", "10**20", "3"]


def scalars_op() -> list:
    s, t, i, j = random.choice("st"), random.choice("st"), random.choice("ij"), random.choice("ij")
    op = scalars_choice(s, t, i, j)
    # (a text stays short: CPython too would run out of memory on what loops make of it)
    return [re.sub(r"^([st]) = (.*)$", r"\1 = (\2)[:999]", op)]


def scalars_choice(s: str, t: str, i: str, j: str) -> str:
    return random.choice([
        f"{s} = {s}.upper()", f"{s} = {s}.lower()", f"{s} = {s}.strip()", f"{s} = {s}.title()", f"{s} = {s}.lstrip('-')",
        f"{s} = {s}.replace({random.choice(SV)}, {random.choice(SV)})", f"{s} = {s} + {t}", f"{s} = {s} * 2", f"{s} = {s}[1:]",
        f"{s} = {s}[::-1]", f"{s} = ','.join({s}.split(','))", f"{s} = '-'.join(sorted({s}))", f"{s} = {s}.zfill(4)",
        f"{s} = {s}.center(6, '*')", f"{s} = f'{{{s}!r}}'", f"{s} = f'{{{s}:>5}}'", f"{s} = '%s|%d' % ({s}, {i})", f"{s} = str({i})",
        f"{s} = repr({i})", f"{s} = format({i}, '05.1f')", f"{i} = len({s})", f"{i} = {s}.count('a')", f"{i} = {s}.find('b')",
        f"{i} = int({s}) if {s}.lstrip('-').isdigit() else -1", f"{i} = {i} + {j}", f"{i} = {i} * {random.choice(NV)}",
        f"{i} = {i} // 3 if {i} else 0", f"{i} = {i} % 4", f"{i} = -{i}", f"{i} = abs({i})", f"{i} = round({i} / 3, 2)",
        f"{i} = {i} ** 2 if abs({i}) < 100 else {i}", f"{i} = max({i}, {j})", f"{i} = min({i}, {j}, 5)", f"{i} = {i} == {j}",
        f"{i} = {s} < {t}", f"{i} = {s}.startswith({random.choice(SV)})", f"{i} = 'b' in {s}", f"{i} = float({i})",
        f"{s} = {s}.split('.')[-1]", f"{s} = {s}.partition(',')[2]", f"{s} = {s}.encode().decode()", f"{s} = chr(65 + len({s}) % 26)",
    ])


def scalars_program():
    head = ["import sys", "n = len(sys.argv)", f"s = {random.choice(SV)}", f"t = {random.choice(SV)}", f"i = {random.choice(NV)}", f"j = {random.choice(NV)}"]
    conds = COND + ["if s:", "if i > 2:"]
    return head, block(scalars_op, 0, random.randrange(4, 14), conds), ["s", "t", "i", "j"]


KINDS = {"lists": lists_program, "objects": objects_program, "calls": calls_program, "scalars": scalars_program}


# ---------------------------------------------------------------- running and checking
def annotate(head: list, body: list, probes: list) -> str | None:
    """The program with its probes and their `#@value includes` directives (the values CPython
    gave over n = 1..5), or None when a run raised."""
    seen: dict = {}

    def rec(k, v):
        seen.setdefault(k, set()).add(repr(v) if isinstance(v, (int, str, float, type(None), bool)) else None)

    lines = head + body
    py = lines + [f"_rec('p{i}', {e})" for i, e in enumerate(probes)]
    saved = sys.argv
    try:
        for n in range(1, 6):
            sys.argv = ["x"] * n
            exec("\n".join(py), {"_rec": rec})  # noqa: S102 (generated code)
    except Exception:  # noqa: BLE001
        return None
    finally:
        sys.argv = saved
    out = list(lines)
    for i, e in enumerate(probes):
        out.append(f"p{i} = {e}")
        vals = seen.get(f"p{i}", set())
        if vals and None not in vals:
            out.append(f"#@value p{i}: includes={{{', '.join(sorted(vals))}}}")
    return "\n".join(out) + "\n"


def fails(path: str) -> list:
    if os.environ.get("FUZZ_TRACE"):
        print(f"checking {path}", file=sys.stderr, flush=True)
    ok, problems = run_case(path)
    return [] if ok else problems


def shrink(path: str, head: list, body: list, probes: list) -> str:
    """Replace statements (and whole blocks) by `pass` while the program still fails."""
    tmp = path.replace(".py", "_try.py")

    def still(b):
        text = annotate(head, b, probes)
        if text is None:
            return False
        with open(tmp, "w") as f:
            f.write(text)
        return any("leaves out" in p or "must not claim" in p or "rowan failed" in p for p in fails(tmp))

    changed = True
    while changed:
        changed = False
        for i, l in enumerate(body):
            if l.strip() in ("pass", "else:", ""):
                continue
            ind = l[: len(l) - len(l.lstrip())]
            j = i + 1
            if l.rstrip().endswith(":"):
                while j < len(body) and (body[j].startswith(ind + " ") or body[j] == ind + "else:"):
                    j += 1
            cand = body[:i] + [ind + "pass"] + [""] * (j - i - 1) + body[j:]
            if still(cand):
                body, changed = cand, True
    os.remove(tmp)
    out = path.replace(".py", "_min.py")
    with open(out, "w") as f:
        f.write(annotate(head, [l for l in body if l], probes))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("kinds", nargs="*", default=list(KINDS))
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("-n", type=int, default=100, help="programs per kind")
    ap.add_argument("-j", type=int, default=4)
    ap.add_argument("--out", default=None)
    ap.add_argument("--shrink", action="store_true")
    a = ap.parse_args()
    out = a.out or tempfile.mkdtemp(prefix="rowan-fuzz-")
    os.makedirs(out, exist_ok=True)
    cases = []
    for kind in a.kinds:
        for k in range(a.n):
            seed = a.seed + k
            random.seed(f"{kind}{seed}")
            if os.environ.get("FUZZ_TRACE"):
                print(f"making {kind} {seed}", file=sys.stderr, flush=True)
            head, body, probes = KINDS[kind]()
            text = annotate(head, body, probes)
            if text is None:
                continue
            path = os.path.join(out, f"{kind}_{seed}.py")
            with open(path, "w") as f:
                f.write(text)
            cases.append((path, head, body, probes))
    # (processes: the golden harness keeps a case's expectations in module globals)
    with ProcessPoolExecutor(a.j) as ex:
        results = list(ex.map(fails, [c[0] for c in cases]))
    bad = [(c, r) for c, r in zip(cases, results) if r]
    for (path, head, body, probes), problems in bad:
        print(f"FAIL {path}")
        for p in [p for p in problems if "value" in p][:4] or problems[:1]:
            print(f"    {p.splitlines()[0][:200]}")
        if a.shrink:
            print(f"    shrunk: {shrink(path, head, body, probes)}")
    print(f"{len(cases) - len(bad)}/{len(cases)} programs agree with CPython ({out})")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
