# Unpacking a sequence of another length raises ValueError (a list, a tuple, a generator's
# items, a string); the names keep their values.
import sys


def counter(k):
    i = 0
    while i < k:
        yield i
        i += 1


n = len(sys.argv)
y = 5
try:
    y, w = counter(3)
except ValueError:
    pass
r1 = y                                       #@value r1: exactly={5}
try:
    a, b = [1, 2, 3]
    r = "ok"
except ValueError:
    r = "ve"
r2 = r                                       #@value r2: exactly={"ve"}
t = (1, 2) if n > 2 else (1, 2, 3)
try:
    a, b = t
    r = "ok"
except ValueError:
    r = "ve"
r3 = r                                       #@value r3: exactly={"ok", "ve"}
try:
    a, b = "abc"
    r = "ok"
except ValueError:
    r = "ve"
r4 = r                                       #@value r4: exactly={"ve"}
a, b = "xy"
r5 = b                                       #@value r5: exactly={"y"}
# (in loops of unknown count the generator object stands for several: its length is not
# known, the unpacking may raise)
y = 5
for _i in range(n - 1):
    for _j in range(n - 1):
        try:
            y, w = counter(3)
        except ValueError:
            y = -1
r6 = y                                       #@value r6: includes={5, -1}
