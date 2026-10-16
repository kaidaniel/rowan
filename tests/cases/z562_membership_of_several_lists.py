# `v in xs` where xs is one of several lists: on the path where it is false, v is not in
# the list xs is, which may be one without v. And a comprehension's filter that may drop an
# item leaves the result's length open.
import json
import sys

n = len(sys.argv)


def pick(xs, v):
    if v in xs:
        return xs
    return [v]


xs = [1, 2]
if n > 2:
    xs = [1]
r = pick(xs, 2)
head = r[0]                                  #@value head: exactly={1, 2}

allowed = {1}
item = json.loads(sys.argv[1])
kept = [w for w in [item] if w in allowed]
nk = len(kept)                               #@value nk: exactly={0, 1}
