"""The keys of a `**kw` dict are strs (Python takes no other keyword names: `f(**{1: 2})` raises
TypeError), for any call and at each call, while the function's code neither changes the dict
nor gives it away; one the code changes, or gives to a function that may, may get keys of any
type. (Before, the keys of such a dict were unknowns of no type.)

Expectations were written from the program text before running the engine."""


def opts(**kw):
    for k in kw:
        name = k
        #@general name: types={str}
    return len(kw)


def pairs(**kw):
    out = []
    for k, v in kw.items():
        out.append(k)
        key = k
        #@general key: types={str}
    return out


def grows(**kw):
    kw[1] = "one"
    for k in kw:
        g = k
        #@general g: untyped
    return len(kw)


def fill(d):
    d[2] = "two"


def passes(**kw):
    fill(kw)
    for k in kw:
        p = k
        #@general p: untyped


def merged(other, **kw):
    kw.update(other)
    for k in kw:
        m = k
        #@general m: untyped


opts(a=1, b=2)
pairs(x="y")
grows(z=0)
passes(w=1)
merged({3: "c"}, q=1)
