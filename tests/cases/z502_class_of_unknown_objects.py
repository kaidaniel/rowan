# `x.__class__(**params)` (scikit-base's clone) on a value that is one of several objects rowan
# cannot see: the call is one call of an unknown class given the parameters; the objects
# themselves are not given to it (one unknown call per possible object, each walking all the
# parameters reach and the object too, multiplied the work).
#@havoc <= 40


def build(a, b, c, d, e, f, g, h, flag):
    est = a
    for o in (b, c, d, e, f, g, h):
        if flag:
            est = o
    tree = {"k%d" % i: {"v": i} for i in range(16)}
    params = {"tree": tree}
    new = est.__class__(**params)
    n = tree["k0"]  #@ likely k0
    return new, n
