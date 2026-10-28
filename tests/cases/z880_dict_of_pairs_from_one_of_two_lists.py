"""`dict(v)` / `OrderedDict(v)` where v is one list of pairs or another (by a branch): the
dict holds one list's pairs, not both lists' — their keys and order are each list's own.
(Before, the pairs of both lists were stored one after the other: `len` was 2 where one list
gives 1, and `popitem()` gave the first list's last pair.)

Expectations were written from the program text before running the engine."""
import sys
from collections import OrderedDict

c = len(sys.argv) > 1
v = [("a", 1)] if c else [("b", 2), ("a", 3)]
n = len(dict(v))
#@value n: exactly={1, 2}
a = OrderedDict(v)["a"]
#@value a: exactly={1, 3}
last = dict(v).popitem()
#@value last: includes={("a", 1), ("a", 3)}
