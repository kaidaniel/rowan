"""Hypothesis: `Counter` over a literal string has the characters as keys and reading a
missing key is fine (it returns 0); a `defaultdict(list)` filled in a known loop has the
loop's keys, reading it never fails, and a plain `dict` copy of it is closed.

Expectations were written from the program text before running the engine."""

from collections import Counter, defaultdict

c = Counter("abca")                          #@row c: present={a,b,c} definite={a,b,c} closed
# Corrected on re-reading: like `totals["c"]` in i92, a read that cannot fail is `ok`, not
# "no verdict". Counter.__missing__ returns 0 *without* inserting, so the row stays {a,b,c}.
c["z"]                                       #@ ok z
_ = 0                                        #@row c: present={a,b,c} definite={a,b,c} closed
groups = defaultdict(list)
for w in ["x", "y", "x"]:
    groups[w].append(w)
g = dict(groups)                             #@row g: present={x,y} definite={x,y} closed
g["q"]                                       #@ missing q
# Corrected on re-reading (i92 convention): reading a defaultdict inserts the key, so `ok`.
groups["q"]                                  #@ ok q
