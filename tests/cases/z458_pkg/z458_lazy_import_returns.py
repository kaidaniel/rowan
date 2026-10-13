"""Hypothesis: a module imported for the first time inside a function (a lazy import) runs
its body without touching the function's state: the function's earlier `return`s still
count. rowan dropped them (the module run cleared the caller's recorded exits), so the
early return's dict was lost (rich's markup.render returned an instance with no attributes).

Expectations were written from the program text before running the engine."""

import sys


def f(c):
    if c:
        return {"a": 1}
    import z458helper
    return {"b": z458helper.X}


r = f(len(sys.argv) > 5)
a = r.get("a")                               #@value a: exactly={1, None}
b = r.get("b")                               #@value b: exactly={2, None}
