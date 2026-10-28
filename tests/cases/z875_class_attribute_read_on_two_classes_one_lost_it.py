"""`k.v` where k may be class A, whose `v` was deleted, or its subclass B, which holds its own:
on B the read finds 2, on A it raises. (Before, A's AttributeError ended the whole path: only
the handler's value was claimed, the 2 was lost.)

Expectations were written from the program text before running the engine."""
import sys

c = len(sys.argv) > 1


class A:
    v = 1


class B(A):
    v = 2


del A.v
k = A if c else B
try:
    r = k.v
except AttributeError:
    r = "attr"
r_seen = r
#@value r_seen: exactly={2, "attr"}
o = A() if c else B()
try:
    t = type(o).v
except AttributeError:
    t = "attr"
t_seen = t
#@value t_seen: exactly={2, "attr"}
