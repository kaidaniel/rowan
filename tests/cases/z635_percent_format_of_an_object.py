"""`"…%s…" % x` where x is an object (an instance, a value read from the environment) is a str.
Of `os.environ["Z635_W"]` (an object until a read finds the key) rowan gave no value at all, as
if the line never completed (`w` was only "-"); of an instance, an unknown of no known type.

Expectations were written from the program text before running the engine."""
import os


class P:
    def __str__(self):
        return "p"


s = "v=%s" % P()
print(s)  # (a str: which one, below; `types=` asked for an unknown, but __str__ runs)
#@value s: exactly={"v=p"}
os.environ.setdefault("Z635_T", "x")
t = "<%s>" % os.environ["Z635_T"]
print(t)  #@value t: types={str}
u = "%s-%s" % ("a", P())
print(u)  #@value u: exactly={"a-p"}
try:
    w = "<%s>" % os.environ["Z635_W"]
except KeyError:
    w = "-"
print(w)  #@value w: types={str}
n = len(t)
print(n)  #@value n: types={int}
