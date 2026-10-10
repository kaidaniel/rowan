"""Hypothesis: closures made by two calls of the same factory have separate cells: a `nonlocal`
total and a captured dict mutated by one `add` are not seen by the other; a decorator's
per-function state dict (call count, first arguments) is separate for each decorated function
and survives across calls made from different callers. Exact on literal events; events from
input add unknown keys to the second tally (open) and make its total unknown."""
import functools
import json
import os


def make_tally():
    counts = {}
    total = 0

    def add(key, n=1):
        nonlocal total
        counts[key] = counts.get(key, 0) + n
        total += n
        return total

    def snapshot():
        return dict(counts), total
    return add, snapshot


def counted(fn):
    calls = {"n": 0, "args": []}

    @functools.wraps(fn)
    def wrapper(*a, **kw):
        calls["n"] += 1
        calls["args"].append(a[0] if a else None)
        return fn(*a, **kw)
    wrapper.calls = calls
    return wrapper


@counted
def label(kind, n):
    return f"{kind}-{n:03d}"


@counted
def area(w, h=2):
    return w * h


def run(events, add):
    for ev in events:
        add(ev["kind"], ev.get("n", 1))          #@ input kind


if __name__ == "__main__":
    add1, snap1 = make_tally()
    add2, snap2 = make_tally()
    add1("a")
    add1("b", 3)
    last = add2("a", 5)                          #@value last: exactly={5}
    run([{"kind": "a", "n": 2}, {"kind": "c"}], add1)
    c1, t1 = snap1()
    #@row c1: present={a,b,c} definite={a,b,c} closed
    ca = c1["a"]                                 #@value ca: exactly={3} ; t1: exactly={7}
    c2, t2 = snap2()
    #@row c2: present={a} definite={a} absent={b,c} closed
    _ = 0                                        #@value t2: exactly={5}
    l1 = label("x", 7)                           #@value l1: exactly={"x-007"}
    l2 = label("y", n=12)                        #@value l2: exactly={"y-012"}
    a1 = area(3) + area(4, h=5)                  #@value a1: exactly={26}
    nl = label.calls["n"]                        #@value nl: exactly={2}
    first = label.calls["args"][0]               #@value first: exactly={"x"}
    na = area.calls["n"]                         #@value na: exactly={2}
    nm = area.__name__                           #@value nm: exactly={"area"}
    run(json.loads(os.environ.get("EVENTS", '[{"kind": "q", "n": 4}]')), add2)
    #@schema json.loads@L73['*']: always={kind} only={kind}
    c3, t3 = snap2()
    #@row c3: present={a} definite={a} open
    _ = 0                                        #@value t3: unknown
