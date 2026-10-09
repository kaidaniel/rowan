"""Hypothesis: a default argument is evaluated when the function is defined: rebinding the
module name later does not change it.

Expectations were written from the program text before running the engine."""

DEF = {"v": 1}


def f(x=DEF):
    return x


DEF = {"other": 1}
r = f()                                      #@row r: present={v} definite={v} closed
