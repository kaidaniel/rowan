"""Followed packages (`--follow`) get a share of the analysis of their own (work and time), apart
from the program's: when it runs out in the middle of an entry (here `import sharelib`, whose
module code needs more work than the share), the entry is undone and is what it is without
following (an unknown module); from then on no package code runs (`latelib` is not followed,
calls into `cheaplib` do not run), while the program's own code keeps its budgets and its
facts. `#@limits budget=64`: the share is 64 × 4 × 64 = 16,384 of work.

Expectations were written from the program text before running the engine."""
#@argv --follow cheaplib,sharelib,latelib --site-packages site
#@limits budget=64
import cheaplib

v = cheaplib.VALUE                           #@value v: exactly={"cheap"}
r0 = cheaplib.double(3)                      #@value r0: exactly={6}

import sharelib

s = sharelib.NAME                            #@value s: unknown

import latelib

w = latelib.VALUE                            #@value w: unknown
v2 = cheaplib.VALUE                          #@value v2: exactly={"cheap"}
r1 = cheaplib.double(4)                      #@value r1: unknown
REG = {"a": 1}
cheaplib.register(REG, "x")
REG["a"]                                     #@ sound a
REG["x"]                                     #@ sound x


def mine(d):
    total = 0
    for i in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]:
        for j in [1, 2, 3, 4]:
            total += d["k"]
    return total


own = mine({"k": 1})                         #@value own: exactly={64}
row = {"id": 7, "name": "ada"}
name = row["name"]                           #@ ok name
#@value name: exactly={"ada"}
