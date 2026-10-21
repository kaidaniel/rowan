# A `for` over a list its body changes: Python takes each next item from the list as it is
# then, so items appended during the loop are iterated too (a worklist) and a removed one
# shifts the rest. A dict or set whose keys change during a loop over it raises RuntimeError
# at the next step; a dict's values may change.
q = [0]
out = []
for w in q:
    out.append(w)
    if w < 2:
        q.append(w + 1)
a = len(out)
#@value a: includes={3}
#@todo value a: exactly={3}  (rounds over a list growing in its body: not counted)
top = max(out)
#@value top: includes={2}
xs = [1, 2]
seen = []
for v in xs:
    seen.append(v)
    if v == 1:
        xs.append(3)
b = len(seen)
#@value b: includes={3}
#@todo value b: exactly={3}  (as a)
d = {"a": 1}
try:
    for k in d:
        d["b"] = 2
    c = "ok"
except RuntimeError:
    c = "err"
#@value c: exactly={"err"}
s = {1}
try:
    for v in s:
        s.add(v + 1)
    e = "ok"
except RuntimeError:
    e = "err"
#@value e: exactly={"err"}
ys = [1, 2, 3]
for i, y in enumerate(ys):
    ys[i] = y * 10
f = ys[2]
#@value f: exactly={30}
zs = [1, 2, 3, 4]
kept = []
for z in zs:
    if z % 2 == 1:
        zs.remove(z)
    kept.append(z)
g = len(kept)
#@value g: includes={2}
#@todo value g: exactly={2}  (as a)
d2 = {"a": 1}
for k in d2:
    d2[k] = 5
h = d2["a"]
#@value h: exactly={5}
