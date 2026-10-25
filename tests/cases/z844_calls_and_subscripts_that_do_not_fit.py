"""Calls whose arguments do not fit the function's parameters raise TypeError (too many, a
missing one, an unknown keyword, one given twice, `*` of a dict of one key); so do subscripts
of the wrong type (`[1, 2]["a"]`, `[][[1]]`, `"ab"[None]`, an instance of a class without
`__getitem__`), `**` of a non-mapping, `~` / `-` of an instance without the dunder, a format
spec a value rejects; `max("")` raises ValueError. (Found by tests/anycall_protocol.py.)

Expectations were written from the program text before running the engine."""
import sys


class Ob:
    v = 1


def two(a, b):
    return a


def kw_only(a, *, k):
    return k


def t(f):
    try:
        return f()
    except TypeError:
        return "TE"
    except ValueError:
        return "VE"
    except IndexError:
        return "IE"


a1 = t(lambda: two(1))
#@value a1: exactly={"TE"}
a2 = t(lambda: two(1, 2, 3))
#@value a2: exactly={"TE"}
a3 = t(lambda: two(*{"k": 1}))
#@value a3: exactly={"TE"}
a4 = t(lambda: two(1, c=2))
#@value a4: exactly={"TE"}
a5 = t(lambda: two(1, a=2))
#@value a5: exactly={"TE"}
a6 = t(lambda: kw_only(1))
#@value a6: exactly={"TE"}
a7 = t(lambda: two(1, b=2))
#@value a7: exactly={1}
a8 = t(lambda: kw_only(1, k=5))
#@value a8: exactly={5}
a9 = t(lambda: two(**{"a": 1}))
#@value a9: exactly={"TE"}
a0 = t(lambda: two(**{"a": 1, "b": 2}))
#@value a0: exactly={1}
b1 = t(lambda: [1, 2]["a"])
#@value b1: exactly={"TE"}
b2 = t(lambda: [][[1]])
#@value b2: exactly={"TE"}
b3 = t(lambda: "ab"[None])
#@value b3: exactly={"TE"}
b4 = t(lambda: Ob()[0])
#@value b4: exactly={"TE"}
b5 = t(lambda: "ab"[1])
#@value b5: exactly={"b"}
b6 = t(lambda: (1, 2)[1])
#@value b6: exactly={2}
c1 = t(lambda: {**3})
#@value c1: exactly={"TE"}
c2 = t(lambda: two(**[1]))
#@value c2: exactly={"TE"}
c3 = t(lambda: ~Ob())
#@value c3: exactly={"TE"}
c4 = t(lambda: -Ob())
#@value c4: exactly={"TE"}
c5 = t(lambda: ~2.5)
#@value c5: exactly={"TE"}
d1 = t(lambda: f"{None:d}")
#@value d1: exactly={"TE"}
d2 = t(lambda: f"{'ab':d}")
#@value d2: exactly={"VE"}
d3 = t(lambda: f"{7:03d}")
#@value d3: exactly={"007"}
d4 = t(lambda: max(""))
#@value d4: exactly={"VE"}
d5 = t(lambda: max("" if len(sys.argv) > 5 else "ab"))  # (one of two strings: their items not known apart)
#@value d5: includes={"VE", "b"}
d6 = t(lambda: enumerate(3))
#@value d6: exactly={"TE"}
d7 = t(lambda: dict(None))
#@value d7: exactly={"TE"}
