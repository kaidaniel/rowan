"""A generator that returns early on one path and raises after its first item on the other: a
loop that stops at its first item takes it on that path, and the raise is never reached."""
import os

c1 = os.environ.get("C1") == "1"


def g():
    if c1:
        return
    yield 1
    raise ValueError("after the first")


def first_only():
    n = 0
    for v in g():
        n = v
        break
    return n


r = first_only()
#@value r: exactly={0, 1}


def guarded():
    n = 0
    try:
        for v in g():
            n = v
            break
    except ValueError:
        n = 5
    return n


r2 = guarded()
#@value r2: includes={0, 1}
#@todo value r2: exactly={0, 1}  (the raise kept for the item after the first is taken where the loop stops)
