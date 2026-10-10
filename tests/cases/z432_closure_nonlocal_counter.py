"""Hypothesis: a closure's nonlocal counter counts the calls actually made (a loop that skips
some items calls it fewer times), and two closures from two make_counter() calls have separate
cells. Precise: run(["a", "_b", "c"]) gives (2, {a: 1, c: 2}); run([]) gives (0, {}); counters
c1 / c2 end at 2 / 1."""
import json
import os


def make_counter():
    n = 0

    def bump(k, seen):
        nonlocal n
        n += 1
        seen[k] = n
        return n

    def get():
        return n

    return bump, get


def run(keys):
    bump, get = make_counter()
    seen = {}
    for k in keys:
        if k.startswith("_"):
            continue
        bump(k, seen)
    return get(), seen


if __name__ == "__main__":
    total, seen = run(["a", "_b", "c"])
    #@value total: exactly={2}
    _ = 0                                        #@row seen: present={a,c} definite={a,c} absent={_b} closed
    sc = seen["c"]                               #@value sc: exactly={2}
    t0, s0 = run([])
    #@value t0: exactly={0}
    _ = 0                                        #@row s0: present={} definite={} closed
    b1, g1 = make_counter()
    b2, g2 = make_counter()
    junk = {}
    b1("x", junk)
    b1("y", junk)
    b2("z", junk)
    n1 = g1()                                    #@value n1: exactly={2}
    n2 = g2()                                    #@value n2: exactly={1}
    jz = junk["z"]                               #@value jz: exactly={1}
    keys = json.loads(os.environ.get("Z432_KEYS", '["_p", "q"]'))
    tu, su = run(keys)
    #@value tu: includes={1}
