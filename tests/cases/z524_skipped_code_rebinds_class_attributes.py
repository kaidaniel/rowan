"""A call past its budget that assigns a class attribute (through the class or `type(self)`)
may rebind it: the attribute may hold anything after it. One that assigns an attribute of
its receiver (`self.x = ...`) sets the instance's own attribute: the class attribute keeps
its value (and what that value holds).

Expectations were written from the program text before running the engine."""
#@limits budget=1


class K:
    n = 0


def setn(x):
    a = 1
    K.n = x


class J:
    m = 1
    defaults = {"k": 1}

    def bump(self, x):
        a = 1
        type(self).m = x

    def own(self, d):
        a = 1
        self.defaults = d


setn(5)
setn(7)
v = K.n  #@value v: includes={7}
j = J()
j.bump(2)
j.bump(3)
w = J.m  #@value w: includes={3}
j.own({"z": 0})
j.own({"y": 0})
dk = J.defaults["k"]  #@ ok k
#@value dk: exactly={1}
