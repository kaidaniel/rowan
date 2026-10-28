"""`itertools.islice` with a negative bound or a step below 1 and `itertools.batched` with a
size below 1 raise ValueError; `filterfalse(None, xs)` keeps the items that are false
themselves. (Before, islice and batched clamped the bad argument, and filterfalse called
None on each item.)

Expectations were written from the program text before running the engine."""
import itertools

v = [3, 0, 2]
try:
    a = list(itertools.islice(v, -1))
except ValueError:
    a = "ve"
a_seen = a
#@value a_seen: exactly={"ve"}
try:
    b = list(itertools.batched(v, 0))
except ValueError:
    b = "ve"
b_seen = b
#@value b_seen: exactly={"ve"}
falsy = list(itertools.filterfalse(None, v))
n = len(falsy)
#@value n: exactly={1}
first = falsy[0]
#@value first: exactly={0}
