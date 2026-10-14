"""Hypothesis: a branch of a conditional expression (or the last operand of `and` / `or`)
that raises gives no value: `a = None if flag else A(None)` where A's `__init__` asserts its
argument is not None leaves `a` None on every path that goes on."""
import sys


class A:
    def __init__(self, x):
        assert x is not None
        self.x = x


a = None if len(sys.argv) < 5 else A(None)
kind = a is None                                 #@value kind: exactly={True}
b = len(sys.argv) < 5 or A(None)                 #@value b: exactly={True}
