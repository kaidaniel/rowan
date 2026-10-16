# Iterating a number raises TypeError (`for q in 5`, `list(5)`, unpacking): the code after
# it never runs on that path.
import sys

n = len(sys.argv)
try:
    for q in 5:
        pass
    r = "no"
except TypeError:
    r = "te"
r1 = r                                       #@value r1: exactly={"te"}
k = 3 if n > 2 else [1]
try:
    m = list(k)
    r = "ok"
except TypeError:
    r = "te"
r2 = r                                       #@value r2: exactly={"ok", "te"}
try:
    a, b = 7
    r = "no"
except TypeError:
    r = "te"
r3 = r                                       #@value r3: exactly={"te"}
