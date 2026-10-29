"""For any call, a parameter annotated `list[int]` (`set[int]`, `list[list[int]]`) holds ints when
the call starts; what the function itself puts into it afterwards joins in (an append, an
insert, an item store, a slice store, an extend, `+=`, a set's add, a method a value of no
known type may have; a dict's new keys), and anything where code rowan does not see is given it. (Before, the
annotation was trusted beyond the call's start: `v` was claimed `int(?)` only.)

Expectations were written from the program text before running the engine."""


def g(xs: list[int]):
    xs.append("b")
    for x in xs:
        v = x
        #@general v: shows={'b'} types={int}


def h(xs: list[int]):
    xs[0] = "c"
    first = xs[0]
    #@general first: exactly={'c'}
    for y in xs:
        w = y
        #@general w: shows={'c'} types={int}


def k(xs: list[int], d: dict[str, int]):
    xs[:] = ["s"]
    d["n"] = "t"
    for z in xs:
        u = z
        #@general u: shows={'s'} types={int}
    for val in d.values():
        q = val
        #@general q: shows={'t'} types={int}


def m(xs: list[int]):
    xs.extend(["e"])
    last = xs[-1]
    #@general last: shows={'e'} types={int}


def b(xs: list[int]):
    xs += ["p"]
    for x in xs:
        vb = x
        #@general vb: shows={'p'} types={int}


def c(xs: list[int]):
    xs.insert(0, "i")
    vc = xs.pop()
    #@general vc: shows={'i'} types={int}


def d(xs: set[int]):
    xs.add("s")
    for x in xs:
        vd = x
        #@general vd: shows={'s'} types={int}


def e(xs: list[int]):
    xs[1:2] = ["q"]
    ve = xs[1]
    #@general ve: shows={'q'} types={int}


def f(xs: list[list[int]]):
    xs[0].append("n")
    for row in xs:
        for y in row:
            vf = y
            #@general vf: shows={'n'} types={int}
            # (the parts of one container are not told apart: what one is given, every one may hold)


def a(xs: list[int]):
    xs.__iadd__(["u"])
    for x in xs:
        va = x
        #@general va: untyped


def kd(d: dict[str, int]):
    d[5] = "t"
    for key in d:
        kk = key
        #@general kk: shows={5}


g([1])
h([2])
k([3], {"a": 4})
m([5])
b([6])
c([7])
d({8})
e([9, 10, 11])
f([[12]])
a([13])
kd({"a": 14})
