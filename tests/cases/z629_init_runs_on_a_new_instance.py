"""Hypothesis: `__init__` runs on a new instance (as constructing the class, or a subclass that
inherits its `__init__`, makes it), also when the program constructs none: an attribute it reads
before setting it is missing, not "missing on some of the times this line runs" (its run for
any call took an instance an earlier `__init__` had set up). A subclass whose own `__init__` sets
the attribute before `super().__init__()` still has it there.

Expectations were written from the program text before running the engine."""


class Account:
    def __init__(self, owner, rate):
        self.owner = owner
        self.interest = self.balance * rate      #@ missing balance
        self.balance = 0


class Point:
    def __init__(self, x, y):
        self.x = x                               #@row self: present={x} definite={x} closed
        self.y = y

    def total(self):
        return self.x + self.y                   #@ ok x ; ok y


class Base:
    def __init__(self):
        self.size = self.width * 2               #@ possibly width


class Wide(Base):
    def __init__(self):
        self.width = 10
        super().__init__()


class Plain(Base):
    pass
