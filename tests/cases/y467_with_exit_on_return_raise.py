"""`__exit__` of a context manager runs however the with body ends: normally, by `return`, or by
an exception (which it may suppress by returning a true value). It ran only after a body that
ended normally: its effects were missing after a `return` from the body (possibly missing) and
after an exception (`log2["closed"]` definitely missing)."""
import sys


class Tracker:
    def __init__(self, log):
        self.log = log

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.log["closed"] = True
        return False


def work(log, flag):
    with Tracker(log):
        if flag:
            return 1
    return 2


def boom(log):
    with Tracker(log):
        raise ValueError("x")


log = {}
work(log, len(sys.argv) > 1)
print(log["closed"])  #@row log: present={closed} definite={closed} closed
log2 = {}
try:
    boom(log2)
except ValueError:
    pass
print(log2["closed"])  #@row log2: present={closed} definite={closed} closed
