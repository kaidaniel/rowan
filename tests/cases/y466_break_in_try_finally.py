"""`break` / `continue` inside a `try` with a `finally` run the `finally` before leaving the
iteration. The loop took the state at the `break` / `continue` before the `finally` ran, so its
effects were missing after the loop (`d["done"]` read as definitely missing)."""
import sys


def first_true(xs):
    d = {}
    for x in xs:
        try:
            if x:
                break
        finally:
            d["done"] = 1
    return d


def skip_falsy(xs):
    seen = {}
    for x in xs:
        try:
            if not x:
                continue
            seen["kept"] = x
        finally:
            seen["visited"] = True
    return seen


r = first_true([1, 2])
print(r["done"])  #@row r: present={done} definite={done} closed
s = skip_falsy([0, len(sys.argv)])
print(s)  #@row s: present={visited,kept} definite={visited} closed
