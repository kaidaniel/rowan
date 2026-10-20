"""Soundness: a call that does not run (`#@limits budget=0`) applies what its code may change
again only where something changed since the last time; whatever did change is changed again:
a global rebound to a new dict, a new dict stored inside one the code reaches, a list given a
new item. Each `work()` here may run `bump`, `poke` and `push` (CPython runs them: 2, 3, 3).

Expectations were written from the program text before running the engine."""
#@limits budget=0

STATE = {"k": 1}
BOX = {"inner": {"v": 1}}
LOG = [0]


def bump():
    STATE["k"] = 2


def poke():
    BOX["inner"]["v"] = 3


def push():
    LOG[-1] = 3


def work():
    bump()
    poke()
    push()


work()
STATE = {"k": 1}
BOX["inner"] = {"v": 1}
LOG.append(1)
k0 = STATE["k"]                              #@value k0: exactly={1}
work()
k = STATE["k"]                               #@value k: includes={1,2}
v = BOX["inner"]["v"]                        #@value v: includes={1,3}
last = LOG[-1]                               #@value last: includes={1,3}
