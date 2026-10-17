"""A binary operator whose method returns NotImplemented: Python then tries the other operand's
reflected method, else raises TypeError; NotImplemented is never the result. And an operand that
is no instance of the program's classes (an unknown) may have its own `__add__`, which Python
runs first: its result may be anything.

Expectations were written from the program text before running the engine."""
import json
import os


class Money:
    def __init__(self, cents):
        self.cents = cents

    def __add__(self, other):
        if isinstance(other, Money):
            return Money(self.cents + other.cents)
        if isinstance(other, int):
            return Money(self.cents + other)
        return NotImplemented

    def __radd__(self, other):
        return self.__add__(other)


class Tag:
    def __radd__(self, other):
        return "tagged"


m = Money(5) + Money(7)
total = m.cents                           #@value total: exactly={12}
n = Money(1) + 2
n_cents = n.cents                         #@value n_cents: exactly={3}
t = Money(1) + Tag()                      #@value t: exactly={"tagged"}

try:
    v = Money(1) + "x"
    reached = True
except TypeError:
    reached = False
flag = reached                            #@value flag: exactly={False}

data = json.loads(os.environ.get("AMOUNT", "3"))
u = data + Money(4)
ok = u is not NotImplemented              #@value ok: types={bool}
#@todo value ok: exactly={True}   (u may be the unknown's own `__add__`'s result: not decided)
