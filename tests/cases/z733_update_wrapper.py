# `functools.update_wrapper(wrapper, wrapped)` gives back the wrapper with the wrapped
# function's name, doc and attributes (its `__dict__`), as `@functools.wraps` does: click's
# `pass_context` / `pass_obj` decorators. A wrapper rowan cannot see is an unknown call.
import functools


def deco(f):
    def new_func(*a):
        return f(*a)
    return functools.update_wrapper(new_func, f)


def g(v):
    return v + 1


g.tag = "t"
h = deco(g)
n = h.__name__
#@value n: exactly={"g"}
t = h.tag
#@value t: exactly={"t"}
w = h(1)
#@value w: exactly={2}
same = h.__wrapped__ is g
#@value same: includes={True}
