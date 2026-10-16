# A key a dict has on some paths only, at no known place in its order (copied in from a dict
# that got it on some paths), stored again: it may come first or last.
import sys


def pick(x, y):
    return x if n > 2 else y


n = len(sys.argv)
e = {}
g = {}
pick(e, g)["k"] = "s"
f = {**e, 1: [1]}
f["k"] = [1]
first = list(f)[0]                           #@value first: includes={"k", 1}
last = list(f)[-1]                           #@value last: includes={"k", 1}
