"""Accumulating over a literal list of sales: a defaultdict(int) of totals, a plain dict of
counts with `.get`, a Counter (open: its keys are data), per-category lists via setdefault.

Expectations were written from the program text before running the engine."""
from collections import Counter, defaultdict

SALES = [{"cat": "a", "n": 1}, {"cat": "b", "n": 2}, {"cat": "a", "n": 3}]

totals = defaultdict(int)
counts = {}
for s in SALES:
    totals[s["cat"]] += s["n"]
    counts[s["cat"]] = counts.get(s["cat"], 0) + 1
_ = 0                                     #@row totals: present={a,b} definite={a,b} closed
_ = 0                                     #@row counts: present={a,b} definite={a,b} closed
cnt = Counter(s["cat"] for s in SALES)    #@row cnt: present={} definite={} open
per = {}
for s in SALES:
    per.setdefault(s["cat"], []).append(s["n"])
_ = 0                                     #@row per: present={a,b} definite={a,b} closed
# Reading a missing key from a defaultdict inserts it.
totals["c"]                               #@ ok c
_ = 0                                     #@row totals: present={a,b,c} definite={a,b,c} closed
counts["c"]                               #@ missing c
per["b"]                                  #@ ok b
