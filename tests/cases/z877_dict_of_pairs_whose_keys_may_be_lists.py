"""`dict(zip(keys, values))` hashes every key: where `keys` holds lists (here after a failed
`remove`, whose items rowan no longer knows one by one) it raises TypeError. (Before, keys
not known one by one were never hashed: the dict was made.)

Expectations were written from the program text before running the engine."""
c = [[1], [2, 3]]
try:
    c.remove(1)
    r = 0
except ValueError:
    r = "ve"
r1 = r
#@value r1: includes={"ve"}
try:
    d = dict(zip(c, range(3)))
    n = len(d)
except TypeError:
    n = "te"
n_seen = n
#@value n_seen: includes={"te"}
