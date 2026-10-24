"""`zip(gen, xs)` asks the generator for an item each round, and once more in the round where
the shorter `xs` ends: a raise there comes after the loop's first round, one before its first
item before any round."""
import os

B = os.environ.get("B", "")


def late():
    yield 1
    if B == "K":
        raise KeyError("k")


def early():
    if B == "K":
        raise KeyError("k")
    yield 1


def rounds(gen):
    n = 0
    try:
        for a, b in zip(gen(), (10,)):
            n = n + a + b
    except KeyError:
        n = n + 100
    return n


r1 = rounds(late)
#@value r1: exactly={11, 111}
r2 = rounds(early)
#@value r2: includes={11, 100}
#@todo value r2: exactly={11, 100}  (claimed: 11 or any int; the raise before the first item is replayed where n is not known)
