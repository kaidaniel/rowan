# `dict.fromkeys(s)` keeps the order the set iterates in, which Python does not promise:
# any member may come first; the keys themselves are known.
import sys

n = len(sys.argv)
s = {"b", "a", "c"}
d = dict.fromkeys(s)
first = list(d)[0]                           #@value first: includes={"a", "b", "c"}
size = len(d)                                #@value size: exactly={3}
has_a = "a" in d                             #@value has_a: exactly={True}
#@row d: present={a, b, c} definite={a, b, c} closed
e = dict.fromkeys(["y", "x"], 0)
first_e = list(e)[0]                         #@value first_e: exactly={"y"}
