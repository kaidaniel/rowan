"""`x in [1, 2]` compares each item with x (`1 == x`: int's `__eq__` gives NotImplemented, so x's
own `__eq__` runs, reflected) until one is equal; an empty list compares nothing."""


class Tracked:
    def __init__(self):
        self.seen = []

    def __eq__(self, other):
        self.seen.append(other)
        return other == 2

    def __hash__(self):
        return 0


t = Tracked()
found = t in [1, 2, 3]
n = len(t.seen)
#@value n: exactly={2}

u = Tracked()
missing = u in []
m = len(u.seen)
#@value m: exactly={0}
