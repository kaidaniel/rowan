# Two dicts copied from each other in a loop of unknown count: copies of copies stand for
# copies of the first originals (each round made new kinds of copies, without end).
import sys

n = len(sys.argv)
d = {"k": 0}
f = {"m": 1}
for _i in range(n):
    if n > 2:
        f = d.copy()
    d = f.copy()
has_k = "k" in d                             #@value has_k: types={bool}
#@objects <= 40
