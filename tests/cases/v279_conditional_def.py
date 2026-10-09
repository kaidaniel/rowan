"""Hypothesis: a function defined differently in the branches of an unknown test is either
definition.

Expectations were written from the program text before running the engine."""

if unknown_flag:
    def make():
        return {"a": 1}
else:
    def make():
        return {"b": 1}
r = make()                                   #@row r: present={a,b} definite={} closed
