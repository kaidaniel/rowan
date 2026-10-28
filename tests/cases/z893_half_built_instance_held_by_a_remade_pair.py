"""An instance whose `__init__` raises is still the program's where something outside the
constructor holds it. Here the only holder is a pair `enumerate` made again at the same place
(same call chain) in an earlier round, now given the new instance, and kept in a global list:
the raise must not drop the instance's attributes. (Before, the second round's instance lost
them in the raise state: `HOLD[-1][1].v` read as unknown.)

Expectations were written from the program text before running the engine."""
import sys

HOLD = []


class K:
    def __init__(self, v):
        self.v = v
        for p in enumerate([self]):
            pass
        HOLD.append(p)
        if v == 2:
            raise ValueError(v)


def rounds(n):
    seen = []
    for a in [1, 2]:
        try:
            K(a)
        except ValueError:
            pass
        k = HOLD[-1][1]
        got = k.v
        #@value got: exactly={1, 2}
        seen.append(got)
    return seen


rounds(len(sys.argv))
