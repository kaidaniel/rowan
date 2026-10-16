# `log[-1]` after a loop whose calls append to log in a `finally` (and raise to a handler on
# some calls): the last item is one those calls appended, or the one before the loop.
import sys


class E1(Exception):
    pass


def boom(v):
    if v == 1:
        raise E1("one")
    if v == "s":
        raise KeyError(v)
    return v


def guarded(v):
    try:
        return boom(v)
    finally:
        log.append(v)


log = []
n = len(sys.argv)
x = 0
y = 1
z = "s"
try:
    x = guarded(y)
except Exception as e:
    pass
for _i in range(n):
    for _i in range(n - 1):
        log.append(x)
try:
    z = boom(z)
except BaseException:
    z = 0
    log.append("b")
for _i in range(n):
    try:
        x = guarded(z)
    except Exception as e:
        pass
q = log[-1] if log else -9                   #@value q: includes={0}
