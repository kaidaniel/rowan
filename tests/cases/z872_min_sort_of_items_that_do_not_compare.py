"""`sort` / `sorted` / `min` / `max` compare the items: a list beside a number (`[[1], [2], 4]`)
or a str beside a number raises TypeError, also where the items are not known one by one
(appended to lists of other kinds, on paths that hold different lists). (Before, only
literals of different kinds counted: lists and other containers among the items did not.)

Expectations were written from the program text before running the engine."""
import sys

c = len(sys.argv) > 1

nested = [[1], [2, 3]]
nested.append(4)
try:
    m = min(nested)
except TypeError:
    m = "te"
m_seen = m
#@value m_seen: exactly={"te"}
try:
    nested.sort()
    s = "sorted"
except TypeError:
    s = "te"
s_seen = s
#@value s_seen: includes={"te"}

lists = [[2], [1]]
try:
    lists.sort()
    ok = "sorted"
except TypeError:
    ok = "te"
ok_seen = ok
#@value ok_seen: exactly={"sorted"}

mixed = [1, 2] if c else ["x"]
mixed.append(4)
try:
    h = max(mixed)
except TypeError:
    h = "te"
h_seen = h
#@value h_seen: includes={4, "te"}
