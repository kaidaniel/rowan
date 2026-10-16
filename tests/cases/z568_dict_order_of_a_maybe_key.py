# A dict iterates in insertion order. A key set on some paths only and stored again later
# sits where the path put it: first on one path, after the others on another.
import sys

n = len(sys.argv)
e = {}
if n % 2 == 0:
    e["m"] = 1
e["z"] = 2
e["m"] = 3
first = list(e)[0]                           #@value first: exactly={"m", "z"}
