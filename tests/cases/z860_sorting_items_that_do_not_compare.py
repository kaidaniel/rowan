"""`list.sort()` and `sorted()` compare the items: an int beside a str, or None beside anything,
raises TypeError. (Before, sorting such a list was claimed to succeed.)

Expectations were written from the program text before running the engine."""

xs = [1, "a", None]
try:
    xs.sort()
    a = "sorted"
except TypeError:
    a = "type"
got_a = a
#@value got_a: includes={"type"}

try:
    b = sorted([None, None])
    b = "sorted"
except TypeError:
    b = "type"
got_b = b
#@value got_b: includes={"type"}

ok = sorted([3, 1.5, True])
#@value ok: shows=[True, 1.5, 3]
