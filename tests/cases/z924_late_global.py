"""A global the main module binds only further down is not bound yet when a function reading
it runs earlier: NameError (the decorator `late_d` at the `def`), then the program goes on;
called after the binding, the function sees it.

Expectations were written from the program text before running the engine."""


def late_d(fn):
    fn.label = LATE
    return fn


try:
    @late_d
    def g():
        return 1

    res = "decorated"
except NameError:
    res = "NameError"
LATE = "late"
v = res                                      #@value v: exactly={"NameError"}


def after():
    return LATE


w = after()                                  #@value w: exactly={"late"}
