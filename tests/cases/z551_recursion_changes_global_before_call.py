"""A recursive function that deletes a key of a global before it calls itself: a deeper call
reads the global as the call before it left it. Once the calls share one context (from the
third level, called through `go`), rowan joined the heap at the recursive call into the
running activation only on what its arguments reach: the deletion was not seen, and the read
was proven though the fifth level fails.

Expectations were written from the program text before running the engine."""
import os

cfg = {"k": 1}
seen = []


def walk(n, depth):
    seen.append(cfg["k"])                #@ possibly k
    if n > 0:
        if depth >= 3:
            del cfg["k"]
        walk(n - 1, depth + 1)


def go(n):
    walk(n, 0)


try:
    go(int(os.environ.get("Z551_N", "6")))
except KeyError:
    pass
