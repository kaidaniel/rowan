"""Hypothesis: calls inside loop / branch conditions run exactly once per evaluation: `while
q.pop():` shrinks the list once per test, a walrus over `next(it, None)` consumes one item per
test, `elif d.setdefault(k, [])` inserts on its first test only, and `and` / `or` chains skip the
calls they short-circuit. On literal inputs counts, lists and the setdefault dict are exact; on
words from input the bucket keys are unknown (open row)."""
import json
import os

LOG = []


def note(tag, v):
    LOG.append(tag)
    return v


def drain(q):
    n = 0
    while q.pop():
        n += 1
    return n, len(q)


def walk(items):
    it = iter(items)
    seen = []
    while (x := next(it, None)) is not None:
        seen.append(x)
    return seen, list(it)


def bucket(words):
    d = {}
    for w in words:
        if w.startswith("#"):
            continue
        elif d.setdefault(w[0], []):
            d[w[0]].append(w)
        else:
            d[w[0]].append(w.upper())
    return d


def chain(a, b):
    return note("a", a) and note("b", b) or note("c", "z")


if __name__ == "__main__":
    n1, r1 = drain([0, 1, 2])                    #@value n1: exactly={2} ; r1: exactly={0}
    n2, r2 = drain([5, 0, 3])                    #@value n2: exactly={1} ; r2: exactly={1}
    seen, rest = walk([3, 0, 4, None, 7])
    ns = len(seen)                               #@value ns: exactly={3}
    s1 = seen[1]                                 #@value s1: exactly={0}
    left = rest[0]                               #@value left: exactly={7}
    b1 = bucket(["ab", "ac", "#x", "bd"])
    #@row b1: present={a,b} definite={a,b} closed
    ba = b1["a"][0] + b1["a"][1]                 #@value ba: exactly={"ABac"}
    bb = b1["b"]                                 #@ ok b
    bc = b1.get("c")                             #@value bc: exactly={None}
    c1 = chain(1, 2)                             #@value c1: exactly={2}
    c2 = chain(0, 2)                             #@value c2: exactly={"z"}
    c3 = chain(1, 0)                             #@value c3: exactly={"z"}
    trace = "".join(LOG)                         #@value trace: exactly={"abacabc"}
    assert note("s", True)
    nlog = len(LOG)                              #@value nlog: exactly={8}
    b3 = bucket(json.loads(os.environ.get("WORDS", '["xa", "#c", "yb", "xd"]')))
    #@row b3: present={} definite={} open
