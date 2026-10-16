"""A call past its budget that assigns an attribute of what a call gives back rebinds that
attribute of the classes the call may give back: a global class a function returns, one a
global dict holds, through a function returning another's result, any class for a function
giving back its parameter. An instance a class makes,
or a class the function makes on each call, is none the program holds: other classes keep
their attribute (rowan rebound it on every class the code reaches, as for a parameter).

Expectations were written from the program text before running the engine."""
#@limits budget=1


class A:
    mode = "x"


class B:
    mode = "x"


class C:
    mode = "x"


class D:
    mode = "x"


class W:
    mode = "x"


class P:
    mode = "x"


REG = {"a": A}


def pick():
    return B


def lookup(k):
    return REG[k]


def pick_w():
    return W


def wrap():
    return pick_w()


def fresh():
    class E:
        mode = "x"
    return E


def make_c():
    return C()


def ident(c):
    return c


def configure(m):
    a = 1
    pick().mode = m
    lookup("a").mode = m
    wrap().mode = m
    obj = make_c()
    obj.mode = m
    f = fresh()
    f.mode = m


def configure_p(m):
    a = 1
    ident(P).mode = m


configure("x")
configure("y")
configure_p("x")
configure_p("y")
ma = A.mode                              #@value ma: includes={"y"}
mb = B.mode                              #@value mb: includes={"y"}
mw = W.mode                              #@value mw: includes={"y"}
mp = P.mode                              #@value mp: includes={"y"}
mc = C.mode                              #@value mc: exactly={"x"}
md = D.mode                              #@value md: exactly={"x"}
