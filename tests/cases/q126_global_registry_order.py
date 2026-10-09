"""Hypothesis: a module-level dict mutated by several function calls in order has, at each
point, exactly the keys added so far (a later call does not leak backwards).

Expectations were written from the program text before running the engine."""

REG = {}


def register(name, **meta):
    REG[name] = dict(meta)
    return REG[name]


def unregister(name):
    return REG.pop(name, None)


register("a", v=1)
snap1 = dict(REG)                         #@row snap1: present={a} definite={a} closed
register("b", v=2, extra=True)
snap2 = dict(REG)                         #@row snap2: present={a,b} definite={a,b} closed
unregister("a")
_ = 0                                     #@row REG: present={b} definite={b} closed
b = REG["b"]                              #@row b: present={v,extra} definite={v,extra} closed
snap1["b"]                                #@ missing b
REG["a"]                                  #@ missing a
