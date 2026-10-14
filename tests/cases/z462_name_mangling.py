"""Hypothesis: a name `__x` written inside a class `C` is `_C__x` (private-name mangling,
done when the class is compiled): `self.__x` is the attribute `_C__x`, which code may name
in full (`"_C__x" in self.__dict__`, `obj._C__x`), and `super().__setattr__` stores it.
hypothesis's ThreadLocal tests `"_ThreadLocal__initialized" in self.__dict__`: rowan read
`self.__kwargs` as never set.

Expectations were written from the program text before running the engine."""


class Counter:
    def __init__(self, start):
        self.__count = start
        self.__initialized = True

    def bump(self):
        self.__count += 1
        return self.__count

    def __setattr__(self, name, value):
        if "_Counter__initialized" not in self.__dict__:
            super().__setattr__(name, value)
        else:
            super().__setattr__(name, value)


c = Counter(5)
n = c.bump()                                 #@value n: exactly={6}
raw = c._Counter__count                      #@ ok _Counter__count
#@value raw: exactly={6}
has = "_Counter__count" in vars(c)           #@value has: exactly={True}
