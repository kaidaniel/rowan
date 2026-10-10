"""An iterator whose `__next__` is decided call by call gives its items in order; one whose
`__next__` changes other objects, raises something else than StopIteration, or stops on an
unknown condition is not unrolled (its items are then any number of what `__next__` gives)."""
import sys


class Count:
    def __init__(self, n):
        self.i = 0
        self.n = n

    def __iter__(self):
        return self

    def __next__(self):
        if self.i >= self.n:
            raise StopIteration
        self.i += 1
        return self.i * 10


xs = [v for v in Count(3)]
t = tuple(xs)
#@value t: exactly={(10, 20, 30)}

LOG = {}


class Noisy:
    def __init__(self):
        self.left = ["a", "b"]

    def __iter__(self):
        return self

    def __next__(self):
        if not self.left:
            raise StopIteration
        x = self.left.pop(0)
        LOG[x] = True
        return x


seen = []
for y in Noisy():
    seen.append("a" in LOG)
# (Noisy changes LOG between items: not unrolled)
ok = all(seen)
#@value ok: includes={True}


class Broken:
    def __iter__(self):
        return self

    def __next__(self):
        raise IndexError("no more")


try:
    for z in Broken():
        pass
    end = "done"
except IndexError:
    end = "index"
#@value end: exactly={"index"}

unk = [v for v in Count(len(sys.argv))]
nu = len(unk)
#@value nu: includes={1, 2, 3}
