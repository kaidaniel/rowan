# `yield from` an iterable of unknown length makes the length of the running activation's
# generator unknown, not the length of the generator's first activation (`g` runs twice here,
# in two contexts).
import json
import sys


def g(src):
    yield 0
    yield from src


a = list(g([1, 2]))
na = len(a)
#@value na: exactly={3}
b = list(g(json.loads(sys.argv[1]) if len(sys.argv) > 1 else [7, 8, 9]))
nb = len(b)
#@value nb: unknown
