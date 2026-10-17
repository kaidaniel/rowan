"""Values of a builtin type whatever the inputs are typed (`int(?)`, `str(?)`): the items of a
`range` with bounds not known (or of more than 64 items), `enumerate`'s index, `os.environ`'s
values, what `input()`, `time.time()`, `random.random()` / `randint`, `math`'s functions, `sum` of
ints and a list's `count` return, and what follows from them (`input().split(",")` is a list of
str, `x == "y"` of a str a bool, `x[:1]` a str). They were `?`.

Expectations were written from the program text before running the engine."""
import math
import os
import random
import sys
import time

n = len(sys.argv)
for i in range(n):
    a = i                               #@value a: types={int}
for k in range(0, 1000, 7):
    c = k                               #@value c: types={int}
#@value c: types={int}
for j, w in enumerate(sys.argv):
    b = j                               #@value b: types={int}
pos = {arg: j for j, arg in enumerate(sys.argv)}
for v in pos.values():
    d = v                               #@value d: types={int}
line = input() if n > 99 else os.environ.get("NOPE", "a,b")
#@value line: types={str}
parts = line.split(",")
first = parts[0]                        #@value first: types={str}
same = first == "a"                     #@value same: types={bool}
t0 = time.time()                        #@value t0: types={float}
r = random.random()                     #@value r: types={float}
dice = random.randint(1, 6)             #@value dice: types={int}
root = math.sqrt(n)                     #@value root: types={float}
lg = math.log(n + 1)                    #@value lg: types={float}
fl = math.floor(root)                   #@value fl: types={int}
nan = math.isnan(root)                  #@value nan: types={bool}
total = sum(len(p) for p in parts)      #@value total: types={int}
cnt = parts.count("a")                  #@value cnt: types={int}
initial = first[:1]                     #@value initial: types={str}
