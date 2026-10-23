# A generator expression given alone to a function that takes every item at once (`sum`,
# `sorted`, `list.extend`, `dict.update`) runs where it is written. Chosen by what the callee
# is, not by its name: a program's own `sum`, `sorted` or method named `update` may take no
# item at all, and the items' effects must not come before the call.
import builtins

LOG = []


def note(i):
    LOG.append(i)
    return i


def sum(it):
    LOG.append("called")
    return 0


r = sum(note(i) for i in range(2))
#@value r: exactly={0}
n1 = len(LOG)
#@value n1: exactly={1}


class Bag:
    def __init__(self):
        self.seen = []

    def update(self, it):
        self.seen.append(len(LOG))
        return self


b = Bag()
LOG.clear()
b.update(note(i) for i in range(3))
n2 = len(LOG)
#@value n2: exactly={0}
s0 = b.seen[0]
#@value s0: exactly={0}


def sorted(xs):
    LOG.append("sorted")
    return []


def shadowed():
    LOG.clear()
    gen = (note(i) for i in range(2))
    out = sorted(gen)
    return len(LOG)


n3 = shadowed()
#@value n3: exactly={1}


def builtin_next():
    LOG.clear()
    gen = (note(i) for i in range(2))
    out = list(gen)
    return len(LOG)


n4 = builtin_next()
#@value n4: exactly={2}

LOG.clear()
t = builtins.sum(note(i) for i in range(3))
#@value t: exactly={3}
n5 = len(LOG)
#@value n5: exactly={3}

d = {}
LOG.clear()
d.update((note(i), i) for i in range(2))
n6 = len(LOG)
#@value n6: exactly={2}
#@row d: present={0,1} definite={0,1} closed
