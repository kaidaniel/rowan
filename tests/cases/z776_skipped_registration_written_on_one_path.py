"""A call that does not run (budget 0) may store a new record into a global registry: the
registry's entry is then an unknown part of it, made where a path first reads it. A store
into it on one path only (`REG["r1"]["a"] = "s"`) left the other path's value out: CPython
gives 1 there (found by the fuzzer, summaries area)."""
#@limits budget=0
import sys

n = len(sys.argv)
REG = {}


def registered(name, v):
    d = {"a": v}
    REG[name] = d
    return d


registered("r2", 0)
d1 = registered("r1", 1)
if n % 2 == 0:
    REG["r1"]["a"] = "s"
p = REG.get("r1", {}).get("a", -9)
#@value p: includes={1}
