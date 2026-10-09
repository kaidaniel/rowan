"""Hypothesis: a class nested in another class is reached as `Outer.Inner` and constructs
normally.

Expectations were written from the program text before running the engine."""


class Outer:
    class Inner:
        def __init__(self):
            self.v = {"inner": True}

    def __init__(self):
        self.child = Outer.Inner()


o = Outer()
v = o.child.v                                #@row v: present={inner} definite={inner} closed
