# `b += [1]` where b is a list on some paths and None on others: the list grows in place where
# it is one; None raises a TypeError, on its own path only.
import sys
n = len(sys.argv)
b = ["s"] if n > 2 else None
caught = 0
try:
    b += [1]
except TypeError:
    caught = 1
cc = caught
#@value cc: exactly={0, 1}
k = len(b) if b is not None else -1
#@value k: includes={2, -1}
#@todo value k: exactly={2, -1}  (the list's state on the path where b is None, length 1, joins in)
c = ["s"]
d = {"k": 0}
if n > 3:
    d[0] = None
c = d.get(0, c)
if n > 1:
    c += [1]
after = 1
#@value after: exactly={1}
