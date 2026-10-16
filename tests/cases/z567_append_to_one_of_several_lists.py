# `c += [x]` or `c.append(x)` where c is one of two lists: either may be the one that grew, so
# a name for the other sees it unchanged on some paths and longer on others.
import sys

n = len(sys.argv)

a = [1, 2]
c = []
if n == 1:
    c = a
c += ["s"]
na = len(a)                                  #@value na: exactly={2, 3}
last = a[-1]                                 #@value last: exactly={2, "s"}

b = [1]
e = [5]
if n > 3:
    e = b
e.append(9)
nb = len(b)                                  #@value nb: exactly={1, 2}
ne = len(e)                                  #@value ne: includes={2}
#@todo value ne: exactly={2}  (which list e is, and whether that one grew, are not related)

# a list of one of two lengths, extended through a name that may be another list
f = ["s"]
if n == 3:
    f.append(0)
g = []
if n == 4:
    g = f
g += [1.5]
nf = len(f)                                  #@value nf: includes={1, 2}
#@todo value nf: exactly={1, 2}  (n == 3 and n == 4 exclude each other)
