"""Hypothesis: a lambda assigned in a class body is a method: called through an instance it
receives the instance.

Expectations were written from the program text before running the engine."""


class C:
    build = lambda self: {"x": 1, "me": self}


c = C()
r = c.build()                                #@row r: present={x,me} definite={x,me} closed
