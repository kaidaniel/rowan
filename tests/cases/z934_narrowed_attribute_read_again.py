"""A test on an attribute (`if c.x:`) tells its later reads on that path what the test saw, but
only while the attribute is what was tested: a property's getter runs again at every read (its
effects count, and it may give something else), and a store through the object's `__dict__`
changes the attribute as a store of it does. A read again of a tested attribute is a read too.

Expectations were written from the program text before running the engine."""
import sys


class Counted:
    def __init__(self, k):
        self.n = 0
        self.k = k

    @property
    def x(self):
        self.n += 1
        return self.k


class Flip:
    def __init__(self):
        self.b = False

    @property
    def on(self):
        self.b = not self.b
        return self.b


class Plain:
    def __init__(self, k):
        self.k = k


def getter_runs_again(c):
    if c.x:
        v1 = c.x
        #@value v1: includes={1}
        #@todo value v1: exactly={1}  (the getter's own read of self.k is not narrowed by the test of c.x)
    n1 = c.n
    # (k 0: the test's read only; k 1: the test's and v1's)
    #@value n1: exactly={1, 2}


def getter_gives_something_else(f):
    if f.on:
        v2 = f.on
        #@value v2: exactly={False}


def through_dict(p):
    if p.k:
        p.__dict__["k"] = 0
        v3 = p.k
        #@value v3: exactly={0}


def through_vars(p):
    if p.k:
        vars(p).update(k=0)
        v4 = p.k
        #@value v4: exactly={0}


def popped_through_dict(p):
    r = "untested"
    if p.k:
        p.__dict__.pop("k")
        try:
            p.k
            r = "kept"
        except AttributeError:
            r = "gone"
    out = r
    #@value out: exactly={"untested", "gone"}


def read_again(p):
    if p.k:
        v6 = p.k                                 #@ ok k
        #@value v6: exactly={1}


arg = 1 if len(sys.argv) > 1 else 0
getter_runs_again(Counted(arg))
getter_gives_something_else(Flip())
through_dict(Plain(arg))
through_vars(Plain(arg))
popped_through_dict(Plain(arg))
read_again(Plain(arg))
