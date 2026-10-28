"""A property is a data descriptor: it takes reads and stores over the instance's own value,
as long as the class holds it. Replaced on the class (`A.v = 9`) or deleted (`del A.v`), a
store `o.v = 5` goes into the instance and the read finds 5; a property without a setter
refuses the store, and the read still runs the getter.
(Before, the setter ran after the class attribute was replaced, a deleted property still
refused the store, and in the run for any call the read took the stored 5 over the getter.)

Expectations were written from the program text before running the engine."""


class A:
    def __init__(self):
        self.a = 1

    @property
    def v(self):
        return self.a

    @v.setter
    def v(self, x):
        self.a = x * 2


class P:
    def __init__(self):
        self.a = 1

    @property
    def v(self):
        return self.a + 100


o = A()
A.v = 9
o.v = 5
replaced = o.v
#@value replaced: exactly={5}

q = P()
del P.v
q.v = 5
deleted = q.v
#@value deleted: exactly={5}


class R:
    def __init__(self):
        self.a = 1

    @property
    def v(self):
        return self.a + 100


def probe(r: R):
    try:
        r.v = 5
        got = "stored"
    except AttributeError:
        got = "attr"
    seen = r.v
    #@general seen: includes={101}
    return got, seen


probe(R())


class S:
    def __init__(self):
        self.a = 1

    @property
    def v(self):
        return self.a

    @v.setter
    def v(self, x):
        self.a = x * 2


def through_setter():
    s = S()
    s.v = 5
    got = s.v
    #@general got: includes={10}
    S.v = 9
    return got


through_setter()
