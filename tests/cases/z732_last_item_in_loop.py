# A list's last item after a loop of rounds not known that appends a dict made in the round (in
# the loop's body or in a function it calls): the dict of the last round, or the one before
# the loop. The round's dict folds into the loop's summary of that site: the last item is
# that summary, which has "a" (not a dict of no keys). Found by the sweep's summaries area.
import sys

n = len(sys.argv)
CACHE = []
CACHE.append({"a": None})
for _i in range(n - 1):
    CACHE.append({"a": 1})
p = CACHE[-1].get("a", -9)
#@value p: includes={None}
q = CACHE[-1]["a"]  #@ ok a

LOG = []


def cached(v):
    d = {"a": v}
    LOG.append(d)
    return d


cached(None)
for _i in range(n + 1):
    cached(1)
r = LOG[-1].get("a", -9)
#@value r: includes={1}
