# A call of the program's own function past its budget does not run; its text says which keys
# it may remove. A function that removes only named keys (`CFG.pop("k")`, `del d["k"]`) leaves
# the others as they were: there unless code rowan cannot see removed them (`likely`), not
# "maybe there". One that removes keys it cannot name (`d.pop(name)`, `d.clear()`) may remove
# any. (The first two calls of each function use its budget; the third does not run.)
#
# Expectations were written from the program text before running the engine.
#@limits budget=2
import os

CFG = {"k": 1, "m": 2}
N = int(os.environ.get("Z637_N", "2"))
NAME = os.environ.get("Z637_NAME", "q")


def drop_global(n):
    a = 1
    if n > 1:
        CFG.pop("k", None)


def drop_arg(d, n):
    a = 1
    if n > 1:
        del d["k"]


def drop_named(d, n):
    a = 1
    if n > 1:
        d.pop(NAME, None)


drop_global(0)
drop_global(0)
drop_global(N)
try:
    x = CFG["k"]                             #@ possibly k
except KeyError:
    x = 0
w = CFG["m"]                                 #@ likely m

rec = {"k": 1, "m": 2}
drop_arg(rec, 0)
drop_arg(rec, 0)
drop_arg(rec, N)
try:
    y = rec["k"]                             #@ possibly k
except KeyError:
    y = 0
v = rec["m"]                                 #@ likely m

other = {"k": 1, "m": 2}
drop_named(other, 0)
drop_named(other, 0)
drop_named(other, N)
try:
    u = other["m"]                           #@ possibly m
except KeyError:
    u = 0
