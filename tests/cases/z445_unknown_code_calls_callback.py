# Code rowan cannot see may call the program's functions it is given: what they change (a
# global dict here) may have changed after the call.
import timeit

COUNTS = {"n": 0}


def tick():
    COUNTS["n"] += 1
    COUNTS["last"] = True


timeit.timeit(tick, number=1)
last = COUNTS.get("last")  #@value last: includes={True}
n = COUNTS["n"]  #@ sound n
