# A `functools.wraps` wrapper of a function rowan does not know (what a decorator made by a call
# that does not run gave back, `#@limits budget=0`) has the attributes that function has: `tag`,
# here "w". Stored on the wrapper on some paths only, it is that, or what the function had.
#@limits budget=0
import functools
import sys

n = len(sys.argv)


def with_tag(t):
    def deco(fn):
        fn.tag = t
        return fn
    return deco


def memo(fn):
    @functools.wraps(fn)
    def inner(v):
        return fn(v)
    return inner


d1 = with_tag("w")
d2 = with_tag("w")


def f(v):
    return v


g = memo(d2(f))
if n == 1:
    if n % 2 == 0:
        g.tag = "z"
p = getattr(g, "tag", "-")
#@value p: includes={"w"}
