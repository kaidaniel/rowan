"""Functions calling the next level of functions from three call sites each, eight levels
deep: the innermost dict is made 3**8 times. Clones of a callee's objects keep only the
innermost call sites of their context, so each summary holds a bounded number of templates
(before, the whole call chain named each clone and every summary held 3**depth of them, all
cloned again at each call)."""
#@objects <= 2000


def f0(x):
    return {"v": x}


def f1(x):
    return [f0(x), f0(x), f0(x)]


def f2(x):
    return [f1(x), f1(x), f1(x)]


def f3(x):
    return [f2(x), f2(x), f2(x)]


def f4(x):
    return [f3(x), f3(x), f3(x)]


def f5(x):
    return [f4(x), f4(x), f4(x)]


def f6(x):
    return [f5(x), f5(x), f5(x)]


def f7(x):
    return [f6(x), f6(x), f6(x)]


def f8(x):
    return [f7(x), f7(x), f7(x)]


r = f8(5)
v = r[0][0][0][0][0][0][0][0]["v"]
#@value v: exactly=5
w = r[2][1][0][2][1][0][2][1]["v"]
#@value w: exactly=5
