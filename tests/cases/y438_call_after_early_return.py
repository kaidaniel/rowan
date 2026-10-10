"""Hypothesis: a call after an early return does not bring the returned path back: the
callee's end joins only its own returns, so what the early path changed stays on that path
(the record returned later lacks the key the early path added)."""
import sys


def helper(x):
    return 1 if x else 2


def f(flag):
    d = {"k": 0}
    if flag:
        d["extra"] = 1
        return None
    helper(1)
    return d


r = f(len(sys.argv) > 2)
#@row r: present={k} definite={k} absent={extra} closed
