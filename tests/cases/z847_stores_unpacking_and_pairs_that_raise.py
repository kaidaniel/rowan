"""Item stores and `del` on a str, a tuple or by a key a list does not take raise TypeError;
attribute stores on a list, a dict or a number AttributeError; `a, *b = []` ValueError;
`dict("ab")` / `d.update("ab")` ValueError and `d.update(2)` TypeError; `format("", "d")`
ValueError; `dict(**obj)` of an instance without `keys` TypeError. (Found by
tests/anycall_protocol.py.)

Expectations were written from the program text before running the engine."""


class Ob:
    v = 1


def t(f):
    try:
        return f()
    except TypeError:
        return "TE"
    except ValueError:
        return "VE"
    except IndexError:
        return "IE"
    except KeyError:
        return "KE"
    except AttributeError:
        return "AE"


def store0(x):
    x[0] = 9
    return x[0]


def delk(x):
    del x["k"]
    return len(x)


def setv(x):
    x.v = 2
    return x.v


def starred(x):
    a, *b = x
    return len(b)


def upd(x, y):
    x.update(y)
    return len(x)


s1 = t(lambda: store0("ab"))
#@value s1: exactly={"TE"}
s2 = t(lambda: store0((1,)))
#@value s2: exactly={"TE"}
s3 = t(lambda: store0([]))
#@value s3: exactly={"IE"}
s4 = t(lambda: store0({}))
#@value s4: exactly={9}
d1 = t(lambda: delk("ab"))
#@value d1: exactly={"TE"}
d2 = t(lambda: delk([1]))
#@value d2: exactly={"TE"}
d3 = t(lambda: delk({"k": 1}))
#@value d3: exactly={0}
v1 = t(lambda: setv([]))
#@value v1: exactly={"AE"}
v2 = t(lambda: setv({}))
#@value v2: exactly={"AE"}
v3 = t(lambda: setv(Ob()))
#@value v3: exactly={2}
u1 = t(lambda: starred([]))
#@value u1: exactly={"VE"}
u2 = t(lambda: starred([1, 2, 3]))
#@value u2: exactly={2}
p1 = t(lambda: upd({}, "ab"))
#@value p1: exactly={"VE"}
p2 = t(lambda: upd({}, 2))
#@value p2: exactly={"TE"}
p3 = t(lambda: upd({}, ""))
#@value p3: exactly={0}
p4 = t(lambda: dict("ab"))
#@value p4: exactly={"VE"}
f1 = t(lambda: format("", "d"))
#@value f1: exactly={"VE"}
f2 = t(lambda: format(None, "d"))
#@value f2: exactly={"TE"}
k1 = t(lambda: dict(**Ob()))
#@value k1: exactly={"TE"}
