"""A call whose function's budget refused it does not run, but what its code would do to its
arguments still happens: `len(o)`, `str(o)`, `o == 1` and `-o` run the dunders of the
program's instance `o`, which change it (`v`). And what such a call gives back is not the
result of the function's run for any call, where its parameters are no instances of the
program's: there `o + 1` runs no code, here `__add__` stores 7 before `o.v` is read."""
#@limits budget=0


class B:
    v = 0

    def __add__(self, x):
        self.v = 7
        return x

    def __len__(self):
        self.v = 11
        return 1

    def __str__(self):
        self.v = 12
        return "b"

    def __eq__(self, x):
        self.v = 13
        return True

    __hash__ = None

    def __neg__(self):
        self.v = 14
        return 1


def sized(o):
    return len(o)


def shown(o):
    return str(o)


def compared(o):
    return o == 1


def negated(o):
    return -o


def added(o):
    o.v = 2
    o + 1
    return o.v


for f in (sized, shown, compared, negated, added):
    f(B())

o1 = B()
sized(o1)
n = o1.v
#@value n: includes={11}

o2 = B()
shown(o2)
n = o2.v
#@value n: includes={12}

o3 = B()
compared(o3)
n = o3.v
#@value n: includes={13}

o4 = B()
negated(o4)
n = o4.v
#@value n: includes={14}

r = added(B())
#@value r: includes={7}
