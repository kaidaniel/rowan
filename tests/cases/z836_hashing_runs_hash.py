"""Hashing an instance — into a set display, as a dict key, in a lookup — runs its `__hash__`."""
log = []


class Key:
    def __init__(self, v):
        self.v = v

    def __hash__(self):
        log.append("hash")
        return self.v


a, b = Key(1), Key(2)
s = {a, b}
n1 = len(log)
#@value n1: exactly={2}
d = {a: 1}
d[b] = 2
n2 = len(log)
#@value n2: exactly={4}
