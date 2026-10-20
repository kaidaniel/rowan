# An attribute stored on an object rowan cannot see, on some paths only: on the others the
# object has what it had (here a decorator made by a call that does not run, `#@limits
# budget=0`: the wrapper it gives back is not known, its `tag` neither). Found by the fuzzer's
# decorators area (an imported module's `@with_tag("w") def f`).
#@limits budget=0
import sys

n = len(sys.argv)


def with_tag(t):
    def deco(fn):
        fn.tag = t
        return fn
    return deco


d1 = with_tag("w")
d2 = with_tag("w")


def f(v):
    return v


f1 = d2(f)
if n == 1:
    for _i in range(n - 1):
        f1.tag = "z"
p = getattr(f1, "tag", "-")
#@value p: includes={"w"}
if n > 1:
    f1.tag = "y"
q = f1.tag
#@value q: includes={"w", "y"}
