"""Builtins that iterate what they are given raise TypeError for a number or None, as a `for`
does: `max(3)`, `sum(None)`, `any(3)`, `[*3]`, `f(*3)`. A number beside a list may raise or
give the list's items. (Found by tests/anycall_protocol.py: `max(3)` was claimed to be 3.)

Expectations were written from the program text before running the engine."""
import heapq
import sys


def t(f, x):
    try:
        return f(x)
    except TypeError:
        return "TE"


def two(a, b):
    return a


a = t(max, 3)
#@value a: exactly={"TE"}
b = t(min, None)
#@value b: exactly={"TE"}
c = t(sum, 3)
#@value c: exactly={"TE"}
d = t(any, 3)
#@value d: exactly={"TE"}
e = t(all, None)
#@value e: exactly={"TE"}
f = t(lambda x: [*x], 2.5)
#@value f: exactly={"TE"}
g = t(lambda x: two(*x), 3)
#@value g: exactly={"TE"}
h = t(sorted, 3)
#@value h: exactly={"TE"}
k = t(max, 3 if len(sys.argv) > 5 else [4, 7])
#@value k: exactly={"TE", 7}
m = t(sum, None if len(sys.argv) > 5 else [1, 2])
#@value m: exactly={"TE", 3}
# (heapq.nlargest's first argument is a count, not its items)
top = heapq.nlargest(1, [{"w": 3}, {"w": 9}], key=lambda r: r["w"])
w = top[0]["w"]
#@value w: exactly={9}
