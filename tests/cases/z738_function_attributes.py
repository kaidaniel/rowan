# Attributes the program stores on its functions: hasattr and getattr with a default decide
# where it was stored on every path (a function's own attributes are dunders); one stored on
# some paths only may be missing on the others, an AttributeError there.
import sys


def k():
    pass


if len(sys.argv) > 2:
    k.tag = 1
try:
    x = k.tag
except AttributeError:
    x = 0
xx = x
#@value xx: exactly={0, 1}
y = getattr(k, "tag", "-")
#@value y: exactly={"-", 1}


def k2():
    pass


k2.tag = 2
h2 = hasattr(k2, "tag")
#@value h2: exactly={True}
h3 = hasattr(k2, "other")
#@value h3: includes={False}
#@todo value h3: exactly={False}  (an attribute a function lacks: a call that does not run may give it back as an unknown, its stores missing it)
z = getattr(k2, "tag", "-")
#@value z: exactly={2}
w = getattr(k2, "other", "-")
#@value w: includes={"-"}
#@todo value w: exactly={"-"}  (as h3)
