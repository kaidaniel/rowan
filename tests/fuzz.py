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


# ---------------------------------------------------------------- dicts and their key sets
# (`#ACC k` marks an access whose verdict is checked: CPython's runs say whether it may fail;
# `#BAD` the handler that runs when it does)
DV = ["0", "1", "2", '"s"', "None", "[1]"]
# (`kx`: "m", or on other runs an unknown call's result that is "k" — a key beside an unknown object)
DK = ['"k"', '"m"', '"z"', "1", "kx"]
DICTS_HEAD = """import sys
def fill(d, k):
    if k not in d:
        d[k] = 9
    return d[k]
def drop(d, k):
    d.pop(k, None)
def keep(d, ks):
    return {k: d[k] for k in ks if k in d}
def merge(a, b):
    out = dict(a)
    out.update(b)
    return out
def pick(x, y):
    return x if n > 2 else y
n = len(sys.argv)
kx = "m" if n > 2 else (sys.argv[0][:0] + "K").lower()
d = {"k": 0}
e = {}
f = {"k": 1, "m": None}
r = 0"""


def dicts_op() -> list:
    x, y, v, w = random.choice("def"), random.choice("def"), random.choice(DV), random.choice(DV)
    k, k2 = random.choice(DK), random.choice(DK)
    return random.choice([
        [f"{x}[{k}] = {v}"], [f"{x}.pop({k}, None)"], [f"if {k} in {x}:", f"    del {x}[{k}]"], [f"{x}.update({{{k}: {v}}})"],
        [f"{x}.update({y})"], [f"{x} = dict({y})"], [f"{x} = {y}"], [f"{x} = {y}.copy()"], [f"{x}.setdefault({k}, {v})"],
        [f"{x} = {{**{y}, {k}: {v}}}"], [f"{x} = {{kk: vv for kk, vv in {y}.items() if vv != {v}}}"],
        [f"{x} = {{kk: {y}[kk] for kk in {y} if kk != {k}}}"], [f"{x}.clear()"], [f'{x} = dict.fromkeys(["k", {k}], {v})'],
        [f'{x} = dict(zip(["k", {k}], [{v}, {w}]))'], [f"{x} |= {{{k}: {v}}}"], [f"{x} = {y} | {{{k}: {v}}}"], [f"r = fill({x}, {k})"],
        [f"drop({x}, {k})"], [f"{x} = keep({y}, [{k}, {k2}])"], [f"{x} = merge({x}, {y})"], [f"pick({x}, {y})[{k}] = {v}"],
        [f"pick({x}, {y}).pop({k}, None)"], [f"for kk in list({x}):", f"    if kk == {k}:", f"        del {x}[kk]"],
        [f"if {x}.get({k}) is None:", f"    {x}[{k}] = {v}"], [f"{x} = {{{k}: {v}}} if n > 3 else {y}"], [f"r = {x}.get({k}, {v})"],
        ["try:", f"    r = {x}[{k}]  #ACC {k}", "except KeyError:", "    r = -1  #BAD"],
        ["try:", f"    del {x}[{k}]  #ACC {k}", "except KeyError:", "    r = -2  #BAD"],
        ["try:", f"    r = {x}.pop({k})  #ACC {k}", "except KeyError:", "    r = -3  #BAD"],
        ["try:", f"    {x}[{k}] = {x}[{k2}]  #ACC {k2}", "except KeyError:", f"    {x}[{k}] = {v}  #BAD"],
        ["try:", f"    r = {x}.popitem()[0]", "except KeyError:", "    r = -4"],
        [f"for kk, vv in list({x}.items()):", f"    if vv == {v}:", f"        {y}[kk] = {w}"],
        [f"if {k} not in {x}:", f"    {x}[{k}] = len({x})", "else:", f"    {x}[{k}] = -{x}[{k}] if isinstance({x}[{k}], int) else {v}"],
        [f"{x}[{k}] = {{{k2}: {v}}}"], [f"r = sorted({x}, key=str)[0] if {x} else {v}"],
    ])


def dicts_program():
    probes = ["r"]
    for x in "def":
        probes += [f"{k} in {x}" for k in DK] + [f"len({x})", f"{x}.get({random.choice(DK)}, -1)", f"list({x})[0] if {x} else -9",
                                                  f"list({x})[-1] if {x} else -9", ("row", x)]
    return DICTS_HEAD.split("\n"), block(dicts_op, 0, random.randrange(4, 14)), probes


# ---------------------------------------------------------------- attributes of instances and classes
AV = ["0", "1", '"s"', "None", "(3,)"]
ATTRS = ["v", "w", "k", "u"]
ATTRS_HEAD = """import sys
class A:
    k = "A"
    def __init__(self, v):
        self.v = v
    def put(self, name, v):
        setattr(self, name, v)
    def get(self):
        return self.v
    def kind(self):
        return self.k
class B(A):
    k = "B"
    def __init__(self, v):
        super().__init__(v)
        self.w = 0
    def get(self):
        return self.w if self.w else super().get()
class C:
    def __init__(self):
        self.v = "c"
    def __getattr__(self, name):
        return "dyn"
def pick(x, y):
    return x if n > 2 else y
n = len(sys.argv)
# (`ax`: "w", or on other runs an unknown call's result that is "u")
ax = "w" if n > 2 else (sys.argv[0][:0] + "U").lower()
a = A(1)
b = B("s")
r = 0"""


def attrs_op() -> list:
    o, o2, v, at = random.choice("ab"), random.choice("ab"), random.choice(AV), random.choice(ATTRS)
    cl = random.choice("AB")
    return random.choice([
        [f"{o}.{at} = {v}"], [f"{cl}.k = {v}"], [f"{cl}.u = {v}"], [f"r = getattr({o}, {at!r}, {v})"], [f"setattr({o}, {at!r}, {v})"],
        [f"if isinstance({o}, A):", f"    {o}.put({at!r}, {v})"], [f"if hasattr({o}, {at!r}):", f"    r = {o}.{at}"], [f"{o} = {o2}"], [f"{o} = A({v})"], [f"{o} = B({v})"],
        [f"{o} = C()"], [f"r = {o}.get() if isinstance({o}, A) else -5"], [f"r = {o}.kind() if isinstance({o}, A) else -6"],
        [f"{o}.v = {o2}.get() if isinstance({o2}, A) else {v}"], [f"pick({o}, {o2}).{at} = {v}"],
        [f"{o}.__dict__[{at!r}] = {v}"], [f"r = vars({o}).get({at!r}, {v})"], [f"r = type({o}).__name__"], [f"r = isinstance({o}, B)"],
        ["try:", f"    r = {o}.{at}  #ACC {at!r}", "except AttributeError:", "    r = -1  #BAD"],
        ["try:", f"    del {o}.{at}  #ACC {at!r}", "except AttributeError:", "    r = -2  #BAD"],
        [f"if {at!r} in vars({o}):", f"    del {o}.{at}"], [f"r = {cl}.k"], [f"{o}.k = {o}.k + '!' if isinstance({o}.k, str) else 'x'"],
        [f"{o}.items = [{v}]"], [f"r = len(vars({o}))"], [f"if isinstance({o}, A):", f"    {o}.v = {v}"],
        [f"setattr({o}, ax, {v})"], [f"r = getattr({o}, ax, {v})"], [f"if hasattr({o}, ax):", f"    r = {o}.{at}"], [f"{o}.__dict__[ax] = {v}"],
        [f"if isinstance({o}, A):", f"    {o}.put(ax, {v})"], [f"if ax in vars({o}):", f"    delattr({o}, ax)"],
        ["try:", f"    delattr({o}, ax)", "except AttributeError:", "    r = -3"],
    ])


def attrs_program():
    probes = ["r", "A.k", "B.k"]
    for o in "ab":
        probes += [f"getattr({o}, {at!r}, -1)" for at in ATTRS] + [f"{at!r} in vars({o})" for at in ATTRS] + [f"type({o}).__name__", ("row", o)]
    return ATTRS_HEAD.split("\n"), block(attrs_op, 0, random.randrange(4, 14)), probes


# ---------------------------------------------------------------- try / except / else / finally, with
TV = ["0", "1", "2", '"s"', "None"]
TRIES_HEAD = """import sys
import contextlib
class E1(Exception):
    pass
class E2(E1):
    pass
def boom(v):
    if v == 1:
        raise E1("one")
    if v == 2:
        raise E2("two")
    if v == "s":
        raise KeyError(v)
    return v
def guarded(v):
    try:
        return boom(v)
    except E2:
        return "e2"
    finally:
        log.append(v)
def wrap(v):
    try:
        boom(v)
    except E1 as e:
        raise ValueError(str(e)) from e
    return "w"
class Sup:
    def __enter__(self):
        log.append("in")
        return self
    def __exit__(self, t, e, tb):
        log.append("out")
        return t is not None and issubclass(t, E2)
log = []
n = len(sys.argv)
x = 0
y = 1
z = "s\""""


def tries_op() -> list:
    x, y, v = random.choice("xyz"), random.choice("xyz"), random.choice(TV)
    return random.choice([
        ["try:", f"    {x} = boom({y})", "except E1:", f"    {x} = {v}", "except KeyError:", f"    {x} = 'k'"],
        ["try:", f"    {x} = boom({y})", "except E2 as e:", f"    {x} = str(e)", "except Exception:", f"    {x} = {v}"],
        ["try:", f"    {x} = boom({y})", "except (E1, KeyError) as e:", f"    {x} = type(e).__name__"],
        ["try:", f"    {x} = boom({y})", "except Exception:", f"    {x} = {v}", "else:", f"    {y} = {v}", "finally:", f"    log.append({x})"],
        ["try:", "    try:", f"        {x} = boom({y})", "    finally:", f"        {y} = {v}", "except Exception:", "    pass"],
        ["try:", "    try:", f"        {x} = boom({y})", "    except E1:", f"        raise ValueError({x})", "except ValueError:", f"    {y} = {v}", "except KeyError:", "    pass"],
        ["for _i in range(n):", "    try:", f"        if _i == {random.randrange(3)}:", "            continue", f"        {x} = boom(_i)",
         "    except E1:", f"        {y} = _i", "        break", "    finally:", f"        {random.choice('xyz')} = _i"],
        ["try:", f"    {x} = guarded({y})", "except Exception as e:", f"    {x} = repr(e)"],
        ["try:", f"    {x} = wrap({y})", "except ValueError as e:", f"    {x} = e.args[0]", "except KeyError:", f"    {x} = 'kw'"],
        ["try:", "    with Sup():", f"        {x} = boom({y})", f"    {y} = {v}", "except Exception:", f"    {x} = {v}"],
        ["with contextlib.suppress(E1):", f"    {x} = {v}", f"    {x} = boom({y})"],
        ["try:", f"    {x} = boom({y})", "except E1 as e:", f"    {x} = e.args[0]", "except KeyError as e:", f"    {x} = e.args[0]"],
        ["try:", f"    {x} = boom({y})", "except BaseException:", f"    {x} = {v}", "    log.append('b')"],
        ["try:", "    raise E2(" + y + ")", "except E1 as e:", f"    {x} = e.args[0]"],
        ["try:", f"    {x} = [boom(w) for w in ({y}, {v})]", "except Exception:", f"    {x} = {v}"],
        ["try:", f"    {x} = int({y})", "except (TypeError, ValueError):", f"    {x} = {v}"],
        ["i = 0", "while True:", "    i += 1", "    try:", "        if i > n:", "            break", f"        {x} = boom(i)", "    except E1:", "        continue",
         "    finally:", f"        {y} = i"],
        [f"{x} = {v}"], [f"log.append({x})"],
    ])


def tries_program():
    return TRIES_HEAD.split("\n"), block(tries_op, 0, random.randrange(3, 10)), ["x", "y", "z", "len(log)", "log[-1] if log else -9", "log[0] if log else -9"]


# ---------------------------------------------------------------- generators
GENS_HEAD = """import sys
G = [0]
def counter(k):
    i = 0
    while i < k:
        yield i
        i += 1
def watch():
    for _ in range(3):
        yield G[0]
def bump():
    G[0] += 1
    yield G[0]
    G[0] += 10
    yield G[0]
def pairs(xs):
    for i, v in enumerate(xs):
        yield i, v
def chain2(a, b):
    yield from a
    yield from b
def tail(xs):
    if not xs:
        return
    yield xs[-1]
def acc(xs):
    total = 0
    for v in xs:
        total += v
        yield total
def evens(xs):
    for v in xs:
        if v % 2 == 0:
            yield v
        else:
            G[0] = v
n = len(sys.argv)
xs = [1, 2]
x = 0
y = 5"""


def gens_op() -> list:
    x, y, k = random.choice("xy"), random.choice("xy"), random.randrange(4)
    v = random.choice(["0", "1", "2", "7", "-1"])
    return random.choice([
        [f"{x} = next(counter({k}), {v})"], [f"{x} = sum(counter(n))"], [f"{x} = len(list(counter({y} % 5)))"],
        ["g = watch()", f"{x} = next(g)", f"G[0] = {v}", f"{y} = next(g, {v})"], [f"{x} = max(acc(xs), default={v})"], [f"xs.append({v})"],
        [f"{x} = [b for a, b in pairs(xs)][0] if xs else {v}"], [f"{x} = list(chain2(xs, [{v}]))[-1]"], [f"{x} = next(tail(xs), {v})"],
        [f"{x} = sum(w for w in xs if w > {k})"], [f"{x} = int(any(w == {v} for w in xs))"],
        ["for w in counter(n):", f"    if w == {k}:", "        break", f"    {x} = w"], ["g = counter(3)", "next(g)", f"{x} = list(g)[0]"],
        ["try:", f"    {x}, {y} = counter({k})", "except ValueError:", f"    {x} = {v}"], [f"{x} = dict(pairs(xs)).get(0, {v})"],
        [f"{x} = next((w for w in xs if w == {v}), -1)"], ["g = bump()", f"{x} = next(g)", f"{y} = G[0]"], [f"{x} = list(bump())[-1]"],
        [f"{x} = list(evens(xs))[0] if 0 in [w % 2 for w in xs] else {v}"], [f"{x} = G[0]"], [f"{x} = sum(1 for _ in evens(xs))"],
        ["g = (w * 2 for w in xs)", f"{x} = list(g)[-1] if xs else {v}"], [f"{x} = sorted(counter(4), key=lambda w: -w)[0]"],
        [f"{x} = [w for w in chain2(counter({k}), xs)][0]"], [f"{x} = {x} % 100"], [f"xs = list(acc(xs))"], [f"{x} = len(xs)"],
        ["it = iter(xs)", f"{x} = next(it, {v})", f"{y} = next(it, {v})"], [f"{x} = list(zip(counter(n), xs))[-1][0] if n and xs else {v}"],
    ])


def gens_program():
    return GENS_HEAD.split("\n"), block(gens_op, 0, random.randrange(3, 11)), ["x", "y", "G[0]", "len(xs)", "xs[-1] if xs else -9", "xs[0] if xs else -9"]


# ---------------------------------------------------------------- comprehensions
COMPS_HEAD = """import sys
n = len(sys.argv)
xs = [1, 2, 3]
ys = ["a", "b"]
d = {"a": 1, "b": 2}
x = 0
y = "s\""""


def comps_op() -> list:
    k, v = random.randrange(4), random.choice(["0", "1", "2", "5"])
    s = random.choice(['"a"', '"b"', '"c"', '""'])
    return random.choice([
        [f"xs = [w * 2 for w in xs if w != {k}]"], [f"xs = [w for w in xs if w > {k}] or [{v}]"], ["ys = [t.upper() for t in ys]"],
        [f"d = {{kk: vv for kk, vv in d.items() if vv != {k}}}"], ["d = {t: i for i, t in enumerate(ys)}"], ["x = sum(w for w in xs)"],
        ["x = len({w % 2 for w in xs})"], [f"x = [a + b for a in xs for b in xs][-1] if xs else {v}"], ["ys = sorted(ys, key=len)"],
        [f"x = max((w for w in xs), default={v})"], [f"x = min(xs) if xs else {v}"], ['y = "".join(t for t in ys if t)'],
        ['y = ",".join(str(w) for w in xs)'], ["xs = [w for w in xs if w in d.values()]"], ["d = {**d, y: x}"],
        [f"ys = [kk for kk in d if d[kk] > {k}]"], ["x = len([1 for w in xs for t in ys])"], ["xs.append(n)"], ["ys.append(y)"],
        ["d[y] = x"], [f"x = {{w: w for w in xs}}.get({k}, {v})"], ["xs = [*xs, *[w + 1 for w in xs]][:5]"],
        [f"x = sum(1 for kk, vv in d.items() if vv == {k})"], ["xs = list(map(lambda w: w + 1, xs))"], ["xs = list(filter(None, xs))"],
        ["x = int(all(w > 0 for w in xs))"], [f"x = next(iter([q for w in xs if (q := w + 1) > {k}]), {v})"],
        [f"ys = [t for t in ys if t != {s}]"], [f"ys = [t + {s} for t in ys][:3]"], ["xs = [len(t) for t in ys]"],
        ["d = {t: len(t) for t in ys}"], [f"xs = [w for w in xs if w % 2] + [{v}]"], ["x = sum(d.values())"],
        ["xs = sorted({w % 3 for w in xs})"], ["ys = list({t: 0 for t in ys})"], [f"x = [w for w in range(n) if w != {k}][-1] if n > 1 else {v}"],
        ["xs = [w for w in xs if isinstance(w, int)]"], [f"d = {{kk: vv + 1 for kk, vv in d.items()}} if d else {{{s}: {v}}}"],
        ["xs = [[w] * 2 for w in xs][0] if xs else xs"], ["xs = [q for p in [[1], xs] for q in p]"], ["x = x % 1000"],
    ])


def comps_program():
    probes = ["x", "y", "len(xs)", "xs[0] if xs else -9", "xs[-1] if xs else -9", "len(ys)", "ys[0] if ys else -9", "len(d)",
              'd.get("a", -1)', '"b" in d', "list(d)[0] if d else -9", ("row", "d")]
    return COMPS_HEAD.split("\n"), block(comps_op, 0, random.randrange(4, 12)), probes


# ---------------------------------------------------------------- string formatting
FS = ['"ab"', '" Ab c "', '"x,y"', '"3"', '""', '"ÄÖ"', '"a\'b"', '"10"']
FI = ["0", "1", "-3", "7", "255", "10**12", "True"]
FF = ["0.0", "2.5", "-0.5", "0.1", "1e16", "3.14159", "0.125", "1/3", "-0.0", "1e-7", "2.675"]


def formats_choice(s: str, i: str, f: str) -> str:
    return random.choice([
        f's = f"{{{i}:>5}}"', f's = f"{{{f}:.2f}}"', f's = f"{{{i}:+d}}"', f's = f"{{{f}:e}}"', f's = f"{{{f}:g}}"', f's = f"{{{i}:,}}"',
        f's = f"{{{s}:^7}}"', f's = f"{{{s}!r:>9}}"', f's = f"{{{i}:08.3f}}"', f's = f"{{{f}:%}}"', f's = "%5.1f" % {f}', f's = "%-6s|" % {s}',
        f's = "%r" % ({s},)', f's = "%x" % {i}', f's = "%03d" % {i}', f's = "{{}} {{}}".format({s}, {i})', f's = "{{0!r}}-{{1:.1f}}".format({s}, {f})',
        f's = "{{a}}{{b}}".format(a={s}, b={i})', f"s = str({f})", f"s = repr({f})", f"s = str([{s}, {i}])", f"s = repr(({s}, {f}))",
        f"s = str({{{s}: {i}}})", f"s = ascii({s})", f's = format({f}, ".3g")', f's = format({i}, "b")', f"s = str(round({f}, 1))",
        f"s = str({f} * 3)", f's = f"{{{s}[:2]}}{{{i}}}"', f"s = {s}.center(9, '-')", f's = "%s" % [{i}]', f's = "%(a)s" % {{"a": {s}}}',
        f's = f"{{{i}!s:>4}}"', f's = f"{{{f}:.0f}}"', f's = f"{{{f}:.1f}}"', f's = format({f}, ",.2f")', f"s = str({f} / 3)", f"s = str({i} / 4)",
        f's = f"{{{i}:x}}|{{{i}:o}}"', f's = f"{{{i}:#06x}}"', f's = "{{:<4}}|".format({s})', f's = f"{{{f}!r}}"', f"s = str(float({i}))",
        f's = "%.3e" % {f}', f's = "%g" % {f}', f's = f"{{{s}=}}"', f's = "%5s%-5s" % ({s}, {i})', f"s = str(divmod({i}, 3))",
        f"s = str({i} ** 2)", f's = "{{:_}}".format({i})', f's = f"{{{f}:+.2e}}"', f"s = str(complex({f}, 1))", f's = str(int({f}))',
        f"i = {i} * 7 - 3", f"i = {i} // 2", f"i = -{i}", f"i = len({s})", f"i = int({f})", f"i = round({f})", f"f = {f} * 1.1", f"f = {f} / 3",
        f"f = float({i}) / 4", f"f = round({f}, 2)", f"f = -{f}", f"f = {f} + 0.1", f"f = abs({f})", f"s = {s}[:12]", f"s = {s} + {s}",
    ])


def formats_op() -> list:
    op = formats_choice(random.choice(["s", "t"] + FS), random.choice(["i"] + FI), random.choice(["f"] + FF))
    # (the target: s / t, i, f)
    tgt, rhs = op.split(" = ", 1)
    if tgt == "s":
        tgt = random.choice("st")
    if tgt in "st":
        rhs = f"({rhs})[:200]"
    return [f"{tgt} = {rhs}"]


def formats_program():
    head = ["import sys", "n = len(sys.argv)", f"s = {random.choice(FS)}", f"t = {random.choice(FS)}", f"i = {random.choice(FI)}", f"f = {random.choice(FF)}"]
    conds = COND + ["if s:", "if i > 2:", "if f < 1:"]
    return head, block(formats_op, 0, random.randrange(4, 12), conds), ["s", "t", "i", "f"]


# ---------------------------------------------------------------- calls between functions (with budgets)
FUNCS_HEAD = """import sys
G = {"c": 0}
def add(a, b=1):
    return a + b
def twice(fn, v):
    return fn(fn(v))
def inc(v):
    G["c"] += 1
    return v + 1
def kw(a, *, b=2, **rest):
    return a * b + len(rest)
def star(*args):
    return sum(args)
def fact(k):
    return 1 if k <= 1 else k * fact(k - 1)
def even(k):
    return True if k == 0 else odd(k - 1)
def odd(k):
    return False if k == 0 else even(k - 1)
def mk(k):
    def fn(v):
        return v * k
    return fn
def mutate(xs, v):
    xs.append(v)
    return len(xs)
def first(xs, default=None):
    for v in xs:
        return v
    return default
def reset(xs):
    xs.clear()
    G["c"] = 0
class Acc:
    def __init__(self):
        self.total = 0
        self.items = []
    def add(self, v):
        self.total += v
        self.items.append(v)
        return self
    def mean(self):
        return self.total // len(self.items) if self.items else 0
    def last(self):
        return self.items[-1] if self.items else -1
def helper(a, b):
    return add(a, b) if a < b else add(b, a)
def deep(v, k):
    return v if k <= 0 else deep(add(v), k - 1)
def apply(acc, vs):
    for v in vs:
        acc.add(v)
    return acc.last()
n = len(sys.argv)
x = 1
y = 2
xs = []
acc = Acc()"""


def funcs_op() -> list:
    x, y, k = random.choice("xy"), random.choice("xy"), random.randrange(4)
    return random.choice([
        [f"{x} = add({x}, {y})"], [f"{x} = add({x})"], [f"{x} = twice(inc, {x})"], [f"{x} = kw({x}, b={y})"], [f"{x} = kw({x}, c=1, d=2)"],
        [f"{x} = star({x}, {y}, {k})"], [f"{x} = star(*xs)"], [f"{x} = fact({x} % 5)"], [f"{x} = int(even({y} % 6))"], [f"{x} = mk({k})({x})"],
        [f"{x} = mutate(xs, {x})"], [f"{x} = first(xs, {y})"], ["acc.add(x)"], [f"{x} = acc.add({y}).total"], [f"{x} = acc.mean()"],
        [f"{x} = helper({x}, {y})"], [f'{x} = G["c"]'], [f"{x} = (lambda a, b={k}: a - b)({x})"], [f"{x} = max({x}, {y}, key=lambda v: -v)"],
        [f"{x} = sorted([{x}, {y}, {k}], key=lambda v: (v % 2, v))[0]"], ["fn = mk(x % 3)", f"{x} = fn({y})"],
        [f"{x} = [inc(v) for v in xs][-1] if xs else {y}"], [f"{x} = {x} % 1000"], ["reset(xs)"], [f"{x} = deep({x}, {k})"],
        [f"{x} = apply(acc, xs)"], [f"{x} = apply(Acc(), [{x}, {k}])"], [f"{x} = acc.last()"], ["acc = Acc()"], [f"{x} = len(acc.items)"],
        [f"{x} = add(*[{x}, {y}])"], [f"{x} = add(**{{'a': {x}, 'b': {k}}})"], [f"{x} = inc({x}) if {y} > {k} else {y}"],
    ])


def funcs_program():
    conds = COND + ["if x > 2:", "if xs:"]
    probes = ["x", "y", "len(xs)", "xs[-1] if xs else -9", 'G["c"]', "acc.total", "len(acc.items)"]
    return FUNCS_HEAD.split("\n"), block(funcs_op, 0, random.randrange(4, 12), conds), probes


KINDS = {"lists": lists_program, "objects": objects_program, "calls": calls_program, "scalars": scalars_program, "dicts": dicts_program,
         "attrs": attrs_program, "tries": tries_program, "gens": gens_program, "comps": comps_program, "formats": formats_program,
         "funcs": funcs_program}


# ---------------------------------------------------------------- running and checking
# What a soundness failure says (golden harness messages): a value CPython saw and rowan left
# out, a line CPython ran that rowan takes for unreachable, a key set or verdict claim a run
# contradicts, a crash. (Other messages — no record object, a row of no object — are not.)
UNSOUND = ("leaves out", "never executed", "rowan failed", "bad JSON", "of a closed row", "not definite in the program", "is absent", "but rowan says")


def annotate(head: list, body: list, probes: list, limits: str = "") -> str | None:
    """The program with its probes and their directives from CPython's runs over n = 1..5, or
    None when a run raised: `#@value pN: includes={…}` (the values a probe took), `#@row pN:
    possible={…} definite_within={…}` (a `("row", NAME)` probe: the keys of the dict, or the
    attributes of the object, NAME was), and on an access marked `#ACC KEY` (its handler `#BAD`)
    `#@ sound KEY` when it succeeded in a run, `#@ fails KEY` when it failed."""
    seen: dict = {}
    rows: dict = {}
    acc: dict = {}

    def rec(k, v):
        seen.setdefault(k, set()).add(repr(v) if isinstance(v, (int, str, float, type(None), bool)) else None)

    def rec_row(k, v):
        keys = v.keys() if isinstance(v, dict) else vars(v).keys() if hasattr(v, "__dict__") else None
        rows.setdefault(k, []).append(None if keys is None else {repr(x) for x in keys})

    lines, py, keys, last = list(head) + [], [], {}, None
    for i, l in enumerate(head + body):
        if "  #ACC " in l:
            l, key = l.split("  #ACC ")
            last, keys[i] = i, key
            py.append(f"{l}; _acc({i}, 1)")
        elif l.endswith("  #BAD"):
            l = l[: -len("  #BAD")]
            py.append(f"{l}; _acc({last}, 0)")
        else:
            py.append(l)
        if i >= len(head):
            lines.append(l)
    py += [f"_row('p{i}', {e[1]})" if isinstance(e, tuple) else f"_rec('p{i}', {e})" for i, e in enumerate(probes)]
    saved = sys.argv
    steps = [0]

    def tracer(frame, event, arg):
        # (a shrunk program may loop for ever: `while True` without its `break`)
        steps[0] += 1
        if steps[0] > 200_000:
            raise TimeoutError
        return tracer

    try:
        sys.settrace(tracer)
        for n in range(1, 6):
            sys.argv = ["x"] * n
            exec("\n".join(py), {"_rec": rec, "_row": rec_row, "_acc": lambda i, ok: acc.setdefault(i, set()).add(ok)})  # noqa: S102 (generated code)
    except Exception:  # noqa: BLE001
        return None
    finally:
        sys.settrace(None)
        sys.argv = saved
    for i, key in keys.items():
        # (a key of a name, `kx`, has no verdict of its own name)
        if key.isidentifier():
            continue
        tiers = (["sound"] if 1 in acc.get(i, ()) else []) + (["fails"] if 0 in acc.get(i, ()) else [])
        if tiers:
            lines[i] += "  #@ " + " ; ".join(f"{t} {key}" for t in tiers)
    out = ([f"#@limits {limits}"] if limits else []) + lines
    for i, e in enumerate(probes):
        if isinstance(e, tuple):
            out.append(f"p{i} = {e[1]}")
            ks = rows.get(f"p{i}", [])
            if ks and None not in ks:
                out.append(f"#@row p{i}: possible={{{', '.join(sorted(set().union(*ks)))}}} definite_within={{{', '.join(sorted(set.intersection(*ks)))}}}")
            continue
        out.append(f"p{i} = {e}")
        vals = seen.get(f"p{i}", set())
        if vals and None not in vals:
            out.append(f"#@value p{i}: includes={{{', '.join(sorted(vals))}}}")
    return "\n".join(out) + "\n"


def fails(path: str) -> list:
    """The soundness problems rowan's answer for the program at `path` has."""
    if os.environ.get("FUZZ_TRACE"):
        print(f"checking {path}", file=sys.stderr, flush=True)
    ok, problems = run_case(path)
    return [p for p in problems if any(m in p for m in UNSOUND)]


def shrink(path: str, head: list, body: list, probes: list, limits: str) -> str:
    """Replace statements (and whole blocks) by `pass` while the program still fails."""
    tmp = path.replace(".py", "_try.py")

    def still(b):
        text = annotate(head, b, probes, limits)
        if text is None:
            return False
        with open(tmp, "w") as f:
            f.write(text)
        return bool(fails(tmp))

    changed = True
    while changed:
        changed = False
        for i, l in enumerate(body):
            if l.strip() in ("pass", "else:", ""):
                continue
            ind = l[: len(l) - len(l.lstrip())]
            j = i + 1
            if l.rstrip().endswith(":"):
                while j < len(body) and (body[j].startswith(ind + " ") or body[j] in (ind + "else:", ind + "finally:") or body[j].startswith(ind + "except")):
                    j += 1
            cand = body[:i] + [ind + "pass"] + [""] * (j - i - 1) + body[j:]
            if still(cand):
                body, changed = cand, True
    os.remove(tmp)
    out = path.replace(".py", "_min.py")
    with open(out, "w") as f:
        f.write(annotate(head, [l for l in body if l], probes, limits))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("kinds", nargs="*", default=list(KINDS))
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("-n", type=int, default=100, help="programs per kind")
    ap.add_argument("-j", type=int, default=4)
    ap.add_argument("--out", default=None)
    ap.add_argument("--shrink", action="store_true")
    ap.add_argument("--limits", default=";budget=0", help="`;`-separated ROWAN_LIMITS each program is checked under (default: none, and budget=0)")
    a = ap.parse_args()
    out = a.out or tempfile.mkdtemp(prefix="rowan-fuzz-")
    os.makedirs(out, exist_ok=True)
    limits = [x.strip() for x in a.limits.split(";")]
    cases, made = [], {}
    for kind in a.kinds:
        for k in range(a.n):
            seed = a.seed + k
            random.seed(f"{kind}{seed}")
            if os.environ.get("FUZZ_TRACE"):
                print(f"making {kind} {seed}", file=sys.stderr, flush=True)
            head, body, probes = KINDS[kind]()
            made.setdefault(kind, [0, 0])[0] += 1
            for lim in limits:
                text = annotate(head, body, probes, lim)
                if text is None:
                    break
                made[kind][1] += 1
                path = os.path.join(out, f"{kind}_{seed}{'_' + re.sub(r'[^a-z0-9]+', '', lim) if lim else ''}.py")
                with open(path, "w") as f:
                    f.write(text)
                cases.append((kind, path, head, body, probes, lim))
    # (processes: the golden harness keeps a case's expectations in module globals)
    with ProcessPoolExecutor(a.j) as ex:
        results = list(ex.map(fails, [c[1] for c in cases]))
    bad = [(c, r) for c, r in zip(cases, results) if r]
    for (kind, path, head, body, probes, lim), problems in bad:
        print(f"FAIL {path}")
        for p in problems[:4]:
            print(f"    {p.splitlines()[0][:200]}")
        if a.shrink:
            print(f"    shrunk: {shrink(path, head, body, probes, lim)}")
    for kind, (m, c) in made.items():
        nb = sum(1 for cs, _ in bad if cs[0] == kind)
        print(f"  {kind}: {m} programs ({m - c // max(len(limits), 1)} raised under CPython), {c} checks, {nb} unsound")
    print(f"{len(cases) - len(bad)}/{len(cases)} checks agree with CPython ({out})")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
