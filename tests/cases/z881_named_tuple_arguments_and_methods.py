"""A namedtuple (or NamedTuple class) is built by a generated `__init__`: a field given no
argument raises TypeError, as does one argument too many — also through `*seq` of a length
not known beside another argument. Its instances are tuples: `index` and `count` work.
(Before, a missing field was only reported, the call went on; `P(*v, 3)` with v one of two
pairs was taken to fit; `p.index(x)` was an AttributeError.)

Expectations were written from the program text before running the engine."""
import sys
from collections import namedtuple

P = namedtuple("P", "x y")
c = len(sys.argv) > 1
try:
    p = P(1)
    short = "made"
except TypeError:
    short = "te"
short_seen = short
#@value short_seen: exactly={"te"}
v = (1, 2) if c else (3, 4)
try:
    q = P(*v, 3)
    extra = "made"
except TypeError:
    extra = "te"
extra_seen = extra
#@value extra_seen: includes={"te"}
pt = P(5, 6)
where = pt.index(6)
#@value where: types={int}
many = pt.count(5)
#@value many: types={int}
