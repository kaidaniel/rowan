# A value that is one list on some paths and something else on others (an unknown string, a
# list computed without known length): what is computed from it covers both, `len(t)` of
# `[] | sys.argv[1]` is not 0.
import sys

n = len(sys.argv)

t = []
if n > 2:
    t = sys.argv[1]
nt = len(t)                                  #@value nt: unknown

b = [7]
if n > 3:
    b.append(1)
a = []
if n > 2:
    a = b * 2
na = len(a)                                  #@value na: includes={0, 2, 4}
#@todo value na: exactly={0, 2, 4}
