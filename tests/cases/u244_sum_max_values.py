"""Hypothesis: `sum` and `max` over `d.values()` of known numbers are known.

Expectations were written from the program text before running the engine."""

prices = {"a": 2, "b": 3}
total = sum(prices.values())
k = {total: 1}                               #@row k: present={5} definite={5} closed
mx = max(prices.values())
k2 = {mx: 1}                                 #@row k2: present={3} definite={3} closed
