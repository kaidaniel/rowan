"""Hypothesis: a generator object is exhausted by its first full iteration: a second
`list(g)` is empty.

Expectations were written from the program text before running the engine."""

xs = [{"a": 1}, {"b": 2}]
g = (x for x in xs)
first = list(g)
again = list(g)
n1 = {len(first): 1}                         #@row n1: present={2} definite={2} closed
n2 = {len(again): 1}                         #@row n2: present={0} definite={0} closed
