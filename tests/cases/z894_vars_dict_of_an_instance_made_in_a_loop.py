"""`vars(obj)` is obj's own attribute dict: a key stored into it is an attribute of obj.
Made in a loop of rounds not known, the instance and its dict are each one of many; folded
into what stands for all of them, the dict must still be the instance's (before, the link
was lost at the loop's back edge and `c0.x` was claimed to raise AttributeError).

Expectations were written from the program text before running the engine."""
import sys


class C:
    pass


keep = []
for i in range(len(sys.argv) + 2):
    c = C()
    d = vars(c)
    keep.append((c, d))
c0, d0 = keep[0]
d0["x"] = 1
try:
    got = c0.x
except AttributeError:
    got = "missing"
result = got
#@value result: includes={1}
