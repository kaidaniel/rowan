# A class attribute stored on some paths only (`if c: B.u = 1`): an instance's read of it may
# fail, `getattr` with a default may give the default, `hasattr` is not known. `del o.x`
# raises AttributeError when the instance itself lacks x (a class attribute x does not count).
import sys


class A:
    k = "A"


class B(A):
    k = "B"


n = len(sys.argv)
b = B()
if n > 2:
    B.u = 1
got = getattr(b, "u", -1)                    #@value got: exactly={1, -1}
has = hasattr(b, "u")                        #@value has: unknown
try:
    q = b.u                                  #@ possibly u
except AttributeError:
    q = -2
q1 = q                                       #@value q1: exactly={1, -2}
kind = b.k                                   #@ ok k
if n > 3:
    b.w = 1
try:
    del b.w  # (the handler guards it: no verdict) #@ none w
    r = "deleted"
except AttributeError:
    r = "ae"
r1 = r                                       #@value r1: exactly={"deleted", "ae"}
try:
    del b.k  # (the handler guards it: no verdict) #@ none k
    r = "deleted"
except AttributeError:
    r = "ae"
r2 = r                                       #@value r2: exactly={"ae"}
