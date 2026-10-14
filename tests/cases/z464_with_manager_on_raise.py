"""Hypothesis: a with statement's manager stays alive while its body runs, also on the way
out by an exception: `__exit__` then reads the manager's attributes. rowan's collector,
recording the raise inside a callee, dropped the manager (held by no name), and its
`__exit__` read `self.t` as never set (sktime's `_SuppressWarningPattern`).

Expectations were written from the program text before running the engine."""

import sys

seen = []


class CM:
    def __init__(self, t):
        self.t = t

    def __enter__(self):
        return None

    def __exit__(self, exc_type, exc, tb):
        seen.append(self.t)                  #@ ok t


def mk():
    return CM(1)


def risky(d):
    return d["k"]


def f(d):
    with mk():
        return risky(d)


try:
    f({} if len(sys.argv) < 5 else {"k": 1})
except KeyError:
    pass
n = seen[0]                                  #@value n: exactly={1}
