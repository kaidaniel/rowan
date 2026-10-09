"""Hypothesis: sorting `items()` by a value with known literals gives a known first pair, and
`sorted(d, key=d.get, reverse=True)` a known first key.

Expectations were written from the program text before running the engine."""

scores = {"ann": 3, "bob": 1, "cy": 2}
ranked = sorted(scores.items(), key=lambda kv: kv[1])
low_name, low_score = ranked[0]
out = {low_name: low_score}                  #@row out: present={bob} definite={bob} closed
top = sorted(scores, key=scores.get, reverse=True)[0]
best = {top: True}                           #@row best: present={ann} definite={ann} closed
