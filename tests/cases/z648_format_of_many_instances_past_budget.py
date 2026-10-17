# `f"{o}"` of a value that may be any of 24 instances runs `__repr__` on each; past its budget
# each was a call that did not run, one per receiver (a closure walk and a havoc each: sktime's
# `BaseObject.clone` formats `{original}`, any of a thousand estimators: 5425 such calls, 2.6 s).
# Those that would not run make one call for all their receivers, as a method call on several
# receivers does (clone: 0.34 s). What `__repr__` may do still happens: CALLS may change.
#@limits budget=1
import os

CALLS = {"n": 0}


def describe(obj):
    CALLS["n"] = CALLS["n"] + 1
    return "R" + type(obj).__name__


class Base:
    def __repr__(self):
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
o = xs[int(os.environ.get("Z648_K", "3"))]
s = f"{o}"
# ("RC3"; one of "RC0" … "RC23", known to be out of reach past the budget)
#@value s: includes={"RC3"}
n = CALLS["n"]
# (1: __repr__ ran once)
#@value n: includes={1}
# (one call, not 23: 148 visits and 128 row joins before)
#@visits <= 100
#@joins <= 60
