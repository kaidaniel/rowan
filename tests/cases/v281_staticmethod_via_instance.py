"""Hypothesis: a staticmethod called through an instance or the class gets only its own
arguments.

Expectations were written from the program text before running the engine."""


class U:
    @staticmethod
    def make(k):
        return {k: True}


u = U()
r = u.make("a")                              #@row r: present={a} definite={a} closed
r2 = U.make("b")                             #@row r2: present={b} definite={b} closed
