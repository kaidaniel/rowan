"""Hypothesis: a @contextmanager whose yield depends on a branch runs the with body at each
reachable yield: the target is bound to every yielded value, the body's writes join, and
what the generator does after each yield follows; inside a function too."""
import sys
from contextlib import contextmanager

LOG = []


@contextmanager
def cm():
    if len(sys.argv) > 3:
        LOG.append("a")
        yield 1
        LOG.append("after a")
    else:
        yield 2
        LOG.append("after b")


with cm() as v:
    y = v
    d = {"k": v}
top = y                                           #@value top: exactly={1,2}
dv = d["k"]                                       #@value dv: exactly={1,2}
#@row d: present={k} definite={k} closed
n = len(LOG)                                      #@value n: exactly={1,2}

#@value n: includes={1,2}

def use():
    with cm() as w:
        r = w * 10
    return r


res = use()                                       #@value res: exactly={10,20}
