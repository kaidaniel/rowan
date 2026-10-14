"""Hypothesis: the remaining builtins run the dunders Python calls for them (`ascii`, `bin` /
`oct` / `hex`, `round`, `divmod`, `pow`, `dir`, `complex`, `bytes`), and library code that
orders the program's objects (heapq, bisect) may run their comparison methods; a builtin
without a model is not taken to change nothing (it is unknown code), `exit()` / `sys.exit()`
end the path, `locals()` at module level are the module's names, `__import__("os")` is the
module.

Expectations were written from the program text before running the engine."""
import bisect
import heapq
import sys

log = {}


class Num:
    def __init__(self, v):
        self.v = v

    def __repr__(self):
        log["repr"] = 1
        return "Num"

    def __index__(self):
        log["index"] = 1
        return 5

    def __round__(self, n=None):
        log["round"] = n
        return 7

    def __divmod__(self, other):
        log["divmod"] = 1
        return (1, 2)

    def __pow__(self, other, mod=None):
        log["pow"] = 1
        return 8

    def __dir__(self):
        log["dir"] = 1
        return ["v"]

    def __float__(self):
        log["float"] = 1
        return 1.5

    def __lt__(self, other):
        log["lt"] = 1
        return self.v < other.v


x = Num(1)
a = ascii(x)
#@value a: exactly={"Num"}
b = bin(x)
#@value b: exactly={"0b101"}
h = hex(x)
#@value h: exactly={"0x5"}
r = round(x, 2)
#@value r: exactly={7}
digits = log["round"]                     #@ ok round
#@value digits: exactly={2}
d = divmod(x, 3)
p = pow(x, 2)
#@value p: exactly={8}
names = dir(x)
c = complex(x)
seen = log
#@row seen: present={repr,index,round,divmod,pow,dir,float} definite={repr,index,round,divmod,pow,dir,float} closed

heap = []
heapq.heappush(heap, Num(2))
heapq.heappush(heap, Num(1))
compared = log.get("lt")                  #@ none lt
#@value compared: includes={1}
row = sorted([3, 1])
pos = bisect.bisect(row, 2)
#@value pos: exactly={1}

os_mod = __import__("os")
sep = os_mod.sep
here = locals()
seen_here = here["x"]                     #@ ok x


def stop():
    log["before"] = 1
    sys.exit(1)
    log["after"] = 1                      # never runs


if len(sys.argv) > 5:
    stop()
after = log.get("after")
#@value after: exactly={None}
