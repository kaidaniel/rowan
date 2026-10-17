"""A lambda whose body raises on every path never returns: after `f()` in a `try`, only the
handler runs. Rowan took the lambda's body value as returned even where its evaluation
raised, so the path after the call went on too.

Expectations were written from the program text before running the engine."""


def boom():
    raise ValueError("x")


f = lambda: boom()
try:
    f()
    k = "ok"
except ValueError:
    k = "bad"
print(k)  #@value k: exactly={"bad"}

g = lambda d: d["x"]
try:
    g({})
    k2 = "ok"
except KeyError:
    k2 = "bad"
print(k2)  #@value k2: exactly={"bad"}
