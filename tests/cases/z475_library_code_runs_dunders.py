"""Hypothesis: library functions taken to change nothing still run the comparison and
arithmetic methods of the program's objects they are given (`statistics.median_low` sorts
them), and numpy's in-place functions change the Python list they are given
(`np.random.shuffle`): neither keeps what rowan knew.

Expectations were written from the program text before running the engine."""
import statistics

import numpy as np

log = {}


class Score:
    def __init__(self, v):
        self.v = v

    def __lt__(self, other):
        log["lt"] = 1
        return self.v < other.v


best = statistics.median_low([Score(3), Score(1), Score(2)])
compared = log.get("lt")
#@value compared: includes={1}

ys = [1, 2, 3]
np.random.shuffle(ys)
first = ys[0]
#@value first: includes={1,2,3}
