"""A key or attribute stored on some paths only, into an object that has content of its own
(an argument inside a summary, an input record): on the other paths a later read sees that
content, not the stored value. The read took the stored value alone (`d["k"]` was `1`), and
the call applied the store to the argument on every path (`o.k` was `1`)."""
import json
import os
import sys


def f(d, c):
    if c:
        d["k"] = 1
    return d["k"]


class O:
    pass


def g(o, c):
    if c:
        o.k = 1
    return o.k


def h(o, c):
    if c:
        o.k = 1


c = len(sys.argv) > 1
r1 = f({"k": "s"}, c)
#@value r1: includes={"s", 1}
o = O()
o.k = "s"
r2 = g(o, c)
#@value r2: includes={"s", 1}
after_g = o.k
#@value after_g: includes={"s", 1}
p = O()
p.k = "s"
h(p, c)
after_h = p.k
#@value after_h: exactly={"s", 1}

d = json.loads(os.environ.get("CFG", '{"k": "input"}'))
if c:
    d["k"] = 1
v = d["k"]
#@value v: includes={"input", 1}
