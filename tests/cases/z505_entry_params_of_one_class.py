# Two parameters annotated with the same class (a dataclass, a plain class): an entry run takes them as two objects (a
# caller may pass two instances), so a store through one does not replace what the other holds.
from dataclasses import dataclass


@dataclass
class P:
    n: int


def f(a: P, b: P):
    a.n = 1
    b.n = 2
    x = a.n
    #@value x: includes={1}
    return x


class Q:
    def __init__(self):
        self.n = 0


def g(a: Q, b: Q):
    a.n = 1
    b.n = 2
    y = a.n
    #@value y: includes={1}
    return y
