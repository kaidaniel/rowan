"""A Counter of words not known and a defaultdict filled under keys not known are dicts of
typed keys and values: a count is an int, a key of `Counter(words)` a str, an item of
`by_len[n]` a str. They were unknowns of no type (`counts["a"]`, `most_common`'s pairs, a
group's items), and hovers showed `<Counter>` / `<defaultdict>`: now `Counter(dict[str(?),
int(?)])`, `defaultdict(dict[int(?), list[str(?)]])`.

Expectations were written from the program text before running the engine."""
import os
from collections import Counter, defaultdict

words = os.environ.get("Z652_W", "a bb a").split()
counts = Counter(words)
n = counts["a"]
#@value n: types={int}
top = counts.most_common(1)
for word, cnt in top:
    w0 = word
    #@value w0: types={str}
    c0 = cnt
    #@value c0: types={int}
for e in counts.elements():
    e0 = e
    #@value e0: types={str}
by_len = defaultdict(list)
for w in words:
    by_len[len(w)].append(w)
for k in by_len:
    k0 = k
    #@value k0: types={int}
for g in by_len[1]:
    g0 = g
    #@value g0: types={str}
seen = defaultdict(int)
for w in words:
    seen[w] += 1
m = seen["zz"]
#@value m: types={int}
