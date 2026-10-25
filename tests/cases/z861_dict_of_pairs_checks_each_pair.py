"""`dict(pairs)` takes each item as a key and a value: an item of another length raises
ValueError, and a key that is a list raises TypeError (unhashable). (Before, `dict([()])` and
`dict(zip([[1]], [2]))` were claimed to make dicts.)

Expectations were written from the program text before running the engine."""

try:
    d = dict([()])
    a = "made"
except ValueError:
    a = "value"
got_a = a
#@value got_a: includes={"value"}

try:
    e = dict(zip([[1], [2]], [3, 4]))
    b = "made"
except TypeError:
    b = "type"
got_b = b
#@value got_b: exactly={"type"}

f = dict(["ab", ("c", 1)])
keys = list(f)
#@value keys: shows=["a", "c"]
