"""Hypothesis: `x = a or b` with a non-empty dict a is a itself, so stores through x reach a and
not b.

Expectations were written from the program text before running the engine."""

a = {"k": 1}
b = {"other": 1}
x = a or b
x["new"] = 1
_ = 0                                        #@row a: present={k,new} definite={k,new} closed
_ = 1                                        #@row b: present={other} definite={other} closed
