# A call of the program's own function past its budget does not run, but its text says what it
# may do: here it pops or deletes keys of a global and of its argument (the first two calls
# of each function use its budget; the third, which removes when N > 1, does not run). After such a call the
# keys are maybe there (`possibly`), not "there unless unknown code removed them" (`likely`):
# the code that removes them is the program's, not code rowan cannot see. A call that only
# stores keeps them likely.
#
# Expectations were written from the program text before running the engine.
#@limits budget=2
import os

CFG = {"k": 1, "m": 2}
N = int(os.environ.get("Z625_N", "2"))


def drop_global(n):
    a = 1
    if n > 1:
        CFG.pop("k", None)


def drop_arg(d, n):
    a = 1
    if n > 1:
        del d["k"]


def store_arg(d, n):
    a = 1
    if n > 1:
        d["z"] = n


drop_global(0)
drop_global(0)
drop_global(N)
try:
    x = CFG["k"]                             #@ possibly k
except KeyError:
    x = 0

rec = {"k": 1}
drop_arg(rec, 0)
drop_arg(rec, 0)
drop_arg(rec, N)
try:
    y = rec["k"]                             #@ possibly k
except KeyError:
    y = 0

kept = {"k": 1}
store_arg(kept, 0)
store_arg(kept, 0)
store_arg(kept, N)
z = kept["k"]                                #@ sound k
