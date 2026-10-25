"""A function's result for any call (what a skipped call gives back, and its hover) covers
every call the annotations allow: `o: Optional[Foo]` may be None, `h: Base` may be an
instance of a subclass overriding the method; and an unknown argument may be what an operator
rejects: `k in x`, `-x` and `for _ in x` may raise TypeError. (Found by
tests/anycall_protocol.py; under `budget=0` the second calls are skipped.)

Expectations were written from the program text before running the engine."""
from typing import Optional


class Foo:
    def first(self):
        return "foo-first"


class Base:
    def enrich(self, d):
        return d


class B(Base):
    def enrich(self, d):
        return {**d, "b": 1}


def h(o: Optional[Foo]):
    t = o.first() if o is not None else "none"
    #@general t: exactly={"foo-first", "none"}
    return t


def run(x: Base):
    return x.enrich({"id": 1})


def has(k, x):
    try:
        return "yes" if k in x else "no"
    except TypeError:
        return "error"


def neg(a):
    try:
        -a
        return 1
    except TypeError:
        return "error"


def walk(a):
    try:
        for _ in a:
            pass
        return 1
    except TypeError:
        return "error"


def put(x: list):
    try:
        x[0] = 9
        return "ok"
    except IndexError:
        return "short"


#@limits budget=0
p1 = h(Foo())
p2 = run(Base())
p3 = has("k", {"k": 1})
p4 = neg(1)
p5 = walk([1])
p6 = put([1])
r1 = h(None)
#@value r1: includes={"none"}
r2 = run(B())
rb = r2.get("b")
#@value rb: includes={1}
r3 = has("k", None)
#@value r3: includes={"error"}
r4 = neg(None)
#@value r4: includes={"error"}
r5 = walk(None)
#@value r5: includes={"error"}
r6 = put([])
#@value r6: includes={"short"}
