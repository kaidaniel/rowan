# A loop of unknown count whose body tests `isinstance(o.v, int)` and then rebinds `o`: o.v is
# an int, a tuple or None on the first round, `p.v` (an int) on the later ones: p.v grows.
# (From the second round on, o.v is a tuple or a number not known: the test may hold.)
import sys


class Box:
    def __init__(self, v):
        self.v = v


n = len(sys.argv)
o = Box(1)
if n == 7:
    o.v = (3,)
elif n == 8:
    o.v = None
p = Box("s")
p.v = 0
for _ in range(n - 1):
    if isinstance(o.v, int):
        o.v += 1
    o = p
pv = p.v                                     #@value pv: includes={0, 1, 2, 3}
