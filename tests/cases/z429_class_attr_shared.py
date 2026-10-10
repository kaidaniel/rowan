"""Hypothesis: `self.shared[k] = v` with `shared` a class attribute stores into the one class
dict seen by all instances (the instance gets no attribute), while `self.shared = {...}` makes
an instance attribute that shadows it from then on. Precise: Registry.shared accumulates {a, b}
over both setup calls and never gets c or own; vars(r2) has shared only after shadow()."""
import json
import os


class Registry:
    shared = {}

    def __init__(self, name):
        self.name = name

    def register(self, k):
        self.shared[k] = self.name

    def shadow(self):
        self.shared = {"own": 1}


def setup(flag):
    r1 = Registry("one")
    r2 = Registry("two")
    r1.register("a")
    r2.register("b")
    if flag:
        r2.shadow()
        r2.register("c")
    return r1, r2


if __name__ == "__main__":
    p1, p2 = setup(False)
    #@row p2: present={name} definite={name} absent={shared} closed
    s = Registry.shared
    #@row s: present={a,b} definite={a,b} closed
    sb = s["b"]                                  #@value sb: exactly={"two"}
    q1, q2 = setup(True)
    #@row q2: present={name,shared} definite={name,shared} closed
    own = q2.shared
    #@row own: present={own,c} definite={own,c} absent={a,b} closed
    s2 = Registry.shared
    #@row s2: present={a,b} definite={a,b} absent={c,own} closed
    seen = p1.shared is q1.shared                #@value seen: exactly={True}
    flag = json.loads(os.environ.get("Z429_FLAG", 'true'))
    u1, u2 = setup(flag)
    #@row u2: present={name,shared} definite={name} closed
    sa = Registry.shared["a"]                    #@value sa: exactly={"one"}
