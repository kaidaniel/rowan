"""Hypothesis: a chain of conditional expressions with known tests picks one label.

Expectations were written from the program text before running the engine."""


def size(n):
    label = "small" if n < 10 else "medium" if n < 100 else "large"
    return {label: n}


a = size(5)                                  #@row a: present={small} definite={small} closed
b = size(50)                                 #@row b: present={medium} definite={medium} closed
c = size(500)                                #@row c: present={large} definite={large} closed
