"""An operator, subscript or attribute on a value of many instances of one class (bounds from a
list of unknown length, compared pairwise): the class's dunder ran once per instance, and for
`lo < hi` once per instance of `lo` for each of `hi`'s inside it; past a few receivers, the
instances of one class run its code once, for all of them."""
import json
import os


class Bound:
    def __init__(self, n, inclusive):
        self.n = n
        self.inclusive = inclusive

    def __lt__(self, other):
        if self.n == other.n:
            return self.inclusive and not other.inclusive
        return self.n < other.n

    def __eq__(self, other):
        return self.n == other.n and self.inclusive == other.inclusive


def ranges(raw):
    bounds = [Bound(0, True), Bound(1, False), Bound(2, True), Bound(3, False), Bound(4, True), Bound(5, False)]
    for x in raw:
        bounds.append(Bound(x, x > 0))
    out = []
    for lo in bounds:
        for hi in bounds:
            if lo < hi and not lo == hi:
                out.append((lo.n, hi.n))
    return out


r = ranges(json.loads(os.environ.get("Z795_RAW", "[7, 8]")))
first = r[0]
lo_n = first[0]
#@value lo_n: includes={0}
hi_n = first[1]
#@value hi_n: includes={1}
# (body runs: the module's calls, the entry runs, and per round of the two loops one `<` and
# one `==` for the class of `lo`'s instances, not one per instance: about 200; it was 1,581)
#@tabulations <= 400
