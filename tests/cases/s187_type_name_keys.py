"""Hypothesis: `type(obj).__name__` and `cls.__name__` are known strings for instances of
known classes, usable as keys.

Expectations were written from the program text before running the engine."""


class Cat:
    pass


class Dog:
    pass


reg = {}
for a in [Cat(), Dog(), Cat()]:
    reg.setdefault(type(a).__name__, []).append(a)
_ = 0                                        #@row reg: present={Cat,Dog} definite={Cat,Dog} closed
reg["Bird"]                                  #@ missing Bird
names = {c.__name__: c for c in (Cat, Dog)}  #@row names: present={Cat,Dog} definite={Cat,Dog} closed
