# A recursive function that changes a global before it calls itself: an inner call reads the
# global as the outer one left it. The calls from the second level on share one context: a
# read there finds the key in some of them and misses it in others.
import os

cfg = {"k": 1}


def walk(n, depth):
    print(cfg["k"])  #@ possibly k
    if n > 0:
        if depth >= 2:
            del cfg["k"]  #@ possibly k
        walk(n - 1, depth + 1)


seen = {}


def count(n):
    # a key the outer call stores is there for the inner one
    if n > 0:
        seen["mark"] = n
        count(n - 1)
    else:
        print(seen["mark"])  #@ possibly mark


count(int(os.environ.get("Z516_N", "2")))
walk(int(os.environ.get("Z516_N", "5")), 0)
