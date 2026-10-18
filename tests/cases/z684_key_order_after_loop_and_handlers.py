# Keys stored under loops of unknown count and in `except` handlers: `e["m"]` is never there in
# the loops, so the handler stores "z" first; the second try stores "z" again, then "m" goes in
# after it. The keys' order is "z", "m" on every run (rowan may not know it, but must not claim
# the other one: `extend` of a dict whose order is not known puts any key at each place).
import sys
n = len(sys.argv)
e = {}
items = []
for _i in range(n):
    for _j in range(n - 1):
        try:
            e["z"] = e["m"]
        except KeyError:
            e["z"] = 0
try:
    e["z"] = e["k"]
except KeyError:
    e["z"] = 1
e["m"] = n
items.extend(e.keys())
first = items[0]
#@value first: includes={"z"}
second = items[1]
#@value second: includes={"m"}
k = len(items)
#@value k: exactly={2}
