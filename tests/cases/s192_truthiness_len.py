"""Hypothesis: `not d` and `len(d) == n` on closed records with known keys are decided.

Expectations were written from the program text before running the engine."""


def describe(d):
    out = {}
    if not d:
        out["empty"] = True
    if len(d) == 2:
        out["pair"] = True
    return out


e = describe({})                             #@row e: present={empty} definite={empty} closed
p = describe({"a": 1, "b": 2})               #@row p: present={pair} definite={pair} closed
o = describe({"a": 1})                       #@row o: present={} definite={} closed
