"""A `return` inside `with cm():` where `cm` is a `@contextmanager` generator returns from the
function (after the generator's code past its `yield` runs). The with body runs at the
generator's yield; what its `return` recorded was replaced by the generator's own state
afterwards, so the function never returned from inside the with: its result lacked that
value (`r["x"]` read as definitely missing) and, when the body always returned, was empty."""
import contextlib
import sys


@contextlib.contextmanager
def opened(log):
    log.append("open")
    yield {"a": 1}
    log.append("close")


def first(flag, log):
    with opened(log) as d:
        if flag:
            return {"x": d["a"]}
    return {"z": 2}


def always(log):
    with opened(log):
        return {"y": 3}


log = []
r = first(len(sys.argv) > 1, log)
print(r)  #@row r: present={x,z} definite={} closed
s = always(log)
print(s)  #@row s: present={y} definite={y} closed
