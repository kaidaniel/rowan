"""None beside objects in a value (`A() if c else None`, `[1] if c else None`) raises where
Python's operators, calls, subscripts and attribute reads reject None: `x + 1`, `g()`,
`lst[0]`, `len(lst)`, `x.v`, `-lst`, `lst < [3]`. Rowan ran only the objects' part (and calls
of None did not raise): each handler below was unreachable.

Expectations were written from the program text before running the engine."""
import os


class A:
    v = 2

    def __add__(self, o):
        return 5

    def __call__(self):
        return 6


def f():
    return 7


x = A() if os.environ.get("Z624_X") else None
g = f if os.environ.get("Z624_G") else None
lst = [1] if os.environ.get("Z624_L") else None
n = 3 if os.environ.get("Z624_N") else None
seen = []

try:
    r1 = x + 1
except TypeError:
    r1 = "te"
seen.append(r1)  #@value r1: exactly={"te", 5}
try:
    r2 = x()
except TypeError:
    r2 = "te"
seen.append(r2)  #@value r2: exactly={"te", 6}
try:
    r3 = g()
except TypeError:
    r3 = "te"
seen.append(r3)  #@value r3: exactly={"te", 7}
try:
    r4 = lst + [2]
except TypeError:
    r4 = "te"
seen.append(r4)  #@value r4: includes={"te"}
try:
    r5 = lst[0]
except TypeError:
    r5 = "te"
seen.append(r5)  #@value r5: exactly={"te", 1}
try:
    r6 = len(lst)
except TypeError:
    r6 = "te"
seen.append(r6)  #@value r6: exactly={"te", 1}
try:
    r7 = x.v
except AttributeError:
    r7 = "ae"
seen.append(r7)  #@value r7: exactly={"ae", 2}
try:
    r8 = -lst
except TypeError:
    r8 = "te"
seen.append(r8)  #@value r8: exactly={"te"}
try:
    r9 = lst < [3]
except TypeError:
    r9 = "te"
seen.append(r9)  #@value r9: exactly={"te", True}
# (lists of known items are ordered item by item: this was a todo)
try:
    r10 = n + 1
except TypeError:
    r10 = "te"
seen.append(r10)  #@value r10: exactly={"te", 4}
