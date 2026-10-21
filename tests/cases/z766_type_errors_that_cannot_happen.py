"""Hypothesis: builtins and str / list methods given values of the types they take raise no
TypeError, whatever those values hold, so a handler for it never runs there: `len` / `iter` /
`list` / `tuple` / `enumerate` / `zip` of a list (its items unknown), `map` / `filter` of the
program's function over one, `list.append` of anything, `list.extend` by a list, str methods
of a str given str and int arguments, `"-".join` of strs. Each step is a raise point only
where it may raise; the handler gives back the step reached. A wrong type still raises
(`endswith(1)`, `len` of an unknown value, `"-".join` of unknown items), and a function `map`
is given that is not the program's may raise on the items (`int` of a str).

Expectations were written from the program text before running the engine."""
import json
import os

ITEMS = list(json.loads(os.environ.get("Z766_ITEMS", '[1, "a", null]')))
WORDS = os.environ.get("Z766_WORDS", "a b.egg").split()
TEXT = os.environ.get("Z766_TEXT", "pkg.egg")
BLOB = json.loads(os.environ.get("Z766_BLOB", '"text"'))


def keep(x):
    return x is not None


def containers(xs):
    step = 0
    try:
        n = len(xs)
        step = 1
        it = iter(xs)
        step = 2
        ys = list(xs)
        step = 3
        t = tuple(xs)
        step = 4
        e = enumerate(xs, 1)
        step = 5
        z = zip(xs, t)
        step = 6
        m = map(keep, xs)
        step = 7
        f = filter(keep, xs)
        step = 8
        ys.append(it)
        step = 9
        ys.extend(xs)
        step = 10
    except TypeError:
        return -step
    return step


r1 = containers(ITEMS)
#@value r1: exactly={10}


def text_methods(s, words):
    step = 0
    try:
        a = s.endswith(".egg")
        step = 1
        b = s.startswith(("a", "b"), 0)
        step = 2
        c = s.replace(".", "-", 1)
        step = 3
        d = s.split(".", 1)
        step = 4
        e = s.strip()
        step = 5
        g = s.rjust(10, "*")
        step = 6
        h = "-".join(words)
        step = 7
    except TypeError:
        return -step
    return step


r2 = text_methods(TEXT, WORDS)
#@value r2: exactly={7}


def wrong_type(s):
    try:
        s.endswith(1)
    except TypeError:
        return "caught"
    return "no"


r3 = wrong_type(TEXT)
#@value r3: exactly={"caught"}
# (endswith of an int raises on every str)


def unknown_values(v, xs):
    step = 0
    try:
        n = len(v)
        step = 1
        j = "-".join(xs)
        step = 2
    except TypeError:
        return -step
    return step


r4 = unknown_values(BLOB, ITEMS)
#@value r4: includes={0, -1, 2}


def not_the_programs(xs):
    out = []
    try:
        for v in map(int, xs):
            out.append(v)
    except (TypeError, ValueError):
        return "failed"
    return "done"


r5 = not_the_programs(ITEMS)
#@value r5: includes={"failed", "done"}
