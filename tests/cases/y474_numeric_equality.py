"""Comparisons of known numbers follow Python's equality, across int, float and bool:
`1 == 1.0`, `True == 1`, `1.0 in [1, 2]`. Equality compared the literals' keys, so all
three were False and the branch they guard was taken as dead."""
import sys

x = 1.0 if len(sys.argv) > 5 else 1
a = "eq" if x == 1 else "ne"
#@value a: exactly={"eq"}
b = "eq" if True == 1 else "ne"
#@value b: exactly={"eq"}
c = "in" if 1.0 in [1, 2] else "out"
#@value c: exactly={"in"}
d = "eq" if 2 == 2.5 else "ne"
#@value d: exactly={"ne"}
y = 1.0
if y == 1:
    e = "taken"
else:
    e = "not taken"
ee = e
#@value ee: exactly={"taken"}
