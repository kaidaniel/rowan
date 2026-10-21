"""Hypothesis: raises that no handler around catches leave the program through the finally
blocks on their way, whatever their class: the finally (here a manager's `__exit__`) runs once
from their states joined, not once per class (a KeyError, an AttributeError, a TypeError, a
ValueError and unknown code's exceptions: five runs of `__exit__` and of what it calls). What
it sees is unchanged: every point that may raise, and the body's end.

Expectations were written from the program text before running the engine."""
import json
import os
import array

ROWS = json.loads(os.environ.get("Z771_ROWS", '[{"n": [1], "m": "7"}]'))
LOG = []


def note(stage):
    seen = stage
    #@value seen: exactly={1, 2, 3, 4, 5}
    LOG.append(seen)


class Track:
    def __init__(self):
        self.stage = 0

    def __enter__(self):
        return self

    def __exit__(self, et, ev, tb):
        note(self.stage)
        return False


def work(row, stream):
    with Track() as t:
        t.stage = 1
        a = row["n"]
        t.stage = 2
        e = stream.itemsize
        t.stage = 3
        c = len(a)
        t.stage = 4
        d = int(row["m"])
        t.stage = 5
    return d


work(ROWS[0], array.array("i"))
#@tabulations <= 24
