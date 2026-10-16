# `del b.k` where b is one of two objects (a loop of unknown count may have made it `a`): on
# the paths where b is the other one, a keeps its k.
import sys


class A:
    k = "A"


n = len(sys.argv)
a = A()
b = A()
for _i in range(n - 1):
    b = a
a.k = "A!"
try:
    del b.k
except AttributeError:
    pass
k = a.k                                      #@value k: exactly={"A", "A!"}
d = {"m": 1}
e = {"m": 2}
g = d if n > 2 else e
del g["m"]
m = d.get("m", -1)                           #@value m: exactly={1, -1}
