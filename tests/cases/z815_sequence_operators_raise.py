"""Operators a list or tuple does not have raise TypeError: `|`, `-`, `&` of two lists, `|=` on
a tuple; `+` of a list and a tuple. `+` and `*` of the same sequence types work."""
a = [1]
r1 = "ok"
try:
    b = a | [2]
except TypeError:
    r1 = "raised"
#@value r1: exactly={"raised"}

t = (1,)
r2 = "ok"
try:
    t |= (2,)
except TypeError:
    r2 = "raised"
#@value r2: exactly={"raised"}

r3 = "ok"
try:
    c = [1] + (2,)
except TypeError:
    r3 = "raised"
#@value r3: exactly={"raised"}

r4 = "ok"
try:
    d = [1, 2] - [1]
except TypeError:
    r4 = "raised"
#@value r4: exactly={"raised"}

e = [1] + [2]
n = len(e * 2)
#@value n: exactly={4}
