# Stores into a list with known positions: by an index not known, every position may be the
# one stored (it was left untouched); by a negative index, the position from the end; a slice
# assignment splices known items in (else positions and length are not known: the assigned
# list was taken for an item); an append to a list of one of a few lengths lands at each
# one's end.
import os

c = os.environ.get("Z519_C")
j = int(os.environ.get("Z519_J", "1"))
n = [1, 2, 3]
n[j] = 9
n0 = n[0]                                        #@value n0: exactly={1,9}
n[-1] = 7
n2 = n[2]                                        #@value n2: exactly={7}

h = ["x"]
if c:
    h.append("y")
h[0:1] = []
h0 = h[0]                                        #@value h0: includes={"y"}

q = ["x"]
if c:
    q.append("y")
q.append("z")
q1 = q[1]                                        #@value q1: exactly={"y","z"}

p = [1, 2, 3]
p[:] = p[1:]
p0 = p[0]                                        #@value p0: exactly={2}
p[1:2] = ["a", "b"]
np = len(p)                                      #@value np: exactly={3}
p2 = p[2]                                        #@value p2: exactly={"b"}
