"""An object made in an earlier round of a loop and made again (same place, same call chain)
with new parts — the pair `enumerate` hands a loop variable — holds those parts: at the
inner loop's end the collector must not drop the facts of what only that pair holds (the
new instance). (Before, the instance of the second round lost its attributes there: `k.v`
read as unknown, and after `k.w = 3` the instance seemed to have `w` only, so `k.v` was
claimed to raise AttributeError.)

Expectations were written from the program text before running the engine."""
import sys


class K:
    def __init__(self, v):
        self.v = v


def rounds(n):
    out = []
    for a in [1, 2]:
        for p in enumerate([K(a)] * n):
            pass
        k = p[1]
        got = k.v
        #@value got: exactly={1, 2}
        k.w = 3
        try:
            r = k.v
        except AttributeError:
            r = "attr"
        after = r
        #@value after: exactly={1, 2}
        out.append(after)
    return out


rounds(len(sys.argv))
