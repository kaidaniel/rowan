# `o.meta` of a value that may be any of 24 instances runs the property on each; past its budget
# each was a call that did not run, one per receiver (a closure walk and a havoc each: narwhals'
# `obj._metadata` on hundreds of compliant objects, 85k such calls importing scikit-learn
# followed). Those that would not run make one call for all their receivers, as a method call on
# several receivers does. What the property may do still happens: CALLS may change.
#@limits budget=0
import os

CALLS = {"n": 0}


def describe(obj):
    CALLS["n"] = CALLS["n"] + 1
    return "M" + type(obj).__name__


class Base:
    @property
    def meta(self):
        return describe(self)


class C0(Base):
    pass


class C1(Base):
    pass


class C2(Base):
    pass


class C3(Base):
    pass


class C4(Base):
    pass


class C5(Base):
    pass


class C6(Base):
    pass


class C7(Base):
    pass


class C8(Base):
    pass


class C9(Base):
    pass


class C10(Base):
    pass


class C11(Base):
    pass


class C12(Base):
    pass


class C13(Base):
    pass


class C14(Base):
    pass


class C15(Base):
    pass


class C16(Base):
    pass


class C17(Base):
    pass


class C18(Base):
    pass


class C19(Base):
    pass


class C20(Base):
    pass


class C21(Base):
    pass


class C22(Base):
    pass


class C23(Base):
    pass


xs = [C0(), C1(), C2(), C3(), C4(), C5(), C6(), C7(), C8(), C9(), C10(), C11(), C12(), C13(), C14(), C15(), C16(), C17(), C18(), C19(), C20(), C21(), C22(), C23()]
first = xs[0].meta
o = xs[int(os.environ.get("Z699_K", "3"))]
s = o.meta
# ("MC3"; one of "MC0" … "MC23", known to be out of reach past the budget)
#@value s: includes={"MC3"}
n = CALLS["n"]
# (2: the property ran twice)
#@value n: includes={2}
# (one call, not 24: 150 visits and 323 row joins before)
#@visits <= 100
#@joins <= 60
