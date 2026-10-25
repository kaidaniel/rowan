"""A class whose `__setattr__` stores through `object.__setattr__`: where a call of the hook does
not run (`budget=0`), `a.w = 3` may have stored `w`, so reading it gives 3 or what `__getattr__`
gives — not `__getattr__`'s answer alone."""
#@limits budget=0


class Rec:
    def __init__(self, v):
        object.__setattr__(self, "v", v)

    def __getattr__(self, name):
        return 0

    def __setattr__(self, name, value):
        object.__setattr__(self, name, value)


def use():
    a = Rec(1)
    a.w = 3
    r = a.w
    #@general r: includes={3}
    return r


use()
