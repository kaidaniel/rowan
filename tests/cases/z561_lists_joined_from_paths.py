# Lists that differ by path, joined: a slice of one of several lists, a value that is a
# list or an unknown, a list cleared on one path and filled on another. What a later read
# sees covers every path.
import sys

n = len(sys.argv)

# one of three lists, sliced: each list's items are in its copy
c = []
if n == 1:
    c = [7]
elif n == 2:
    c.append(None)
a = c[:]
for x in a:
    va = x                                   #@value va: exactly={7, None}

# a list, or another of a length not known repeated (a value rowan does not follow): its
# items include the other's
grown = [1]
for _ in range(n):
    grown.append(2)
b = ["s"]
if n > 2:
    b = grown * 2
first = b[0]                                 #@value first: includes={"s", 1}
#@todo value first: exactly={"s", 1}
for x in b:
    vb = x                                   #@value vb: includes={"s", 1, 2}
#@todo value vb: exactly={"s", 1, 2}

# cleared, then sorted on one path and filled on another
d = ["s"]
d.clear()
if n > 2:
    d.sort()
else:
    d += ["t"]
for x in d + [0]:
    vd = x                                   #@value vd: exactly={"t", 0}
