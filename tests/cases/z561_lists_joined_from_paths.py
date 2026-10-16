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

# a list, or a list of a length not known repeated, concatenated: the result holds the
# other's items too
g = [5]
for _ in range(n):
    g.append(6)
src = []
if n > 2:
    src = g * 2
cat = src + [0]
for x in cat:
    vcat = x                                 #@value vcat: includes={5, 6, 0}
#@todo value vcat: exactly={5, 6, 0}

# `+=` on a list or a number: the list is extended in place, the number added to
num = [1] if n > 5 else 9
if n <= 5:
    num += 1
nv = num                                     #@value nv: includes={10}

# a copy of a list of one of two lengths, appended to: its first item is either
h = []
if n > 2:
    h.append(0)
h2 = list(h)
h2 += ["s"]
h0 = h2[0]                                   #@value h0: exactly={0, "s"}
