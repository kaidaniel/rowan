"""`a[0] = 1` on a parameter: for any call, `a` may be an object that takes no item store (a
tuple, None, a number), so a handler of TypeError may run."""


def put(a):
    try:
        a[0] = 1
        r = "stored"
    except TypeError:
        r = "TypeError"
    out = r
    #@value out: exactly={"stored"}
    #@general out: includes={"stored", "TypeError"}
    return r


put([0])
