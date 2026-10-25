"""Builtins run their items' dunders: `sorted` compares (`__lt__`), `sum` adds (`0 + x`:
`__radd__`), `str([x])` runs `__repr__`, `xs.count(x)` and `[x] == [y]` run `__eq__`,
`copy.copy` runs `__copy__`, `list(x)` runs `__iter__` (and `__len__` for its size)."""
import copy

log = []


class Item:
    def __init__(self, v):
        self.v = v

    def __lt__(self, o):
        log.append("lt")
        return self.v < o.v

    def __radd__(self, o):
        log.append("radd")
        return o + self.v

    def __repr__(self):
        log.append("repr")
        return "Item"

    def __eq__(self, o):
        log.append("eq")
        return isinstance(o, Item) and self.v == o.v

    def __hash__(self):
        return self.v

    def __copy__(self):
        log.append("copy")
        return Item(self.v)


a, b = Item(1), Item(2)
n0 = len(log)
#@value n0: exactly={0}
sorted([b, a])
n1 = len(log)
#@value n1: includes={1}
#@todo value n1: exactly={1}  (sorted's comparisons are run as library code runs dunders: any number)
sum([a, b])
str([a])
[b, a].count(a)
[a] == [b]
copy.copy(a)
n2 = len(log)
#@value n2: includes={7}
#@todo value n2: exactly={7}  (as n1)
