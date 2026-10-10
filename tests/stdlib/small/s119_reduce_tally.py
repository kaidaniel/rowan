"""Hypothesis: functools.reduce with a helper that mutates and returns its accumulator builds
the dict exactly: with an initializer `{}` the keys are the record kinds seen (sums exact),
an empty list returns the initializer itself (empty), and without an initializer the FIRST
element is the accumulator, so the caller's first dict is mutated and returned (aliasing).
Kinds from the environment give an open row; each record must have `kind`."""
import functools
import json
import os


def bump(acc, key, n):
    acc[key] = acc.get(key, 0) + n
    return acc


def tally(acc, rec):
    return bump(acc, rec["kind"], rec.get("qty", 1))


def totals(records):
    return functools.reduce(tally, records, {})


def merge(a, b):
    a.update(b)
    return a


def merged(dicts):
    return functools.reduce(merge, dicts)


if __name__ == "__main__":
    t1 = totals([{"kind": "a", "qty": 2}, {"kind": "b"}, {"kind": "a", "qty": 3}])
    #@row t1: present={a,b} definite={a,b} closed
    ta = t1["a"]                                 #@value ta: exactly={5}
    tb = t1["b"]                                 #@value tb: exactly={1}
    t2 = totals([])
    #@row t2: present={} definite={} closed
    first = {"x": 1}
    m = merged([first, {"y": 2}, {"x": 3}])
    #@row first: present={x,y} definite={x,y} closed
    fx = first["x"]                              #@value fx: exactly={3}
    same = m is first                            #@value same: exactly={True}
    one = merged([{"z": 0}])
    #@row one: present={z} definite={z} closed
    recs = json.loads(os.environ.get("S119_RECS", '[{"kind": "c", "qty": 4}, {"kind": "d"}]'))
    #@schema json.loads@L47['*']: always={kind} only={kind}
    t3 = totals(recs)
    #@row t3: present={} definite={} open
