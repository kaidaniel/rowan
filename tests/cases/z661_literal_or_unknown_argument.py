# A model's argument that is a literal on one path and an unknown (an unknown call's result)
# on the other: the model computed with the literal alone. python z661.py → k is the script's
# path in capitals (no "a" in it, as rowan cannot know: it may be any text); z661.py x y → k
# is "a". Companion of z659 (stores and deletes with such a key).
import sys
from collections import Counter

n = len(sys.argv)
k = "a" if n > 2 else sys.argv[0].upper()
p1 = "abc".startswith(k)
#@value p1: types={bool}
p2 = "abc".replace(k, "z")
#@value p2: includes={'zbc', 'abc'}
p3 = "abc".partition(k)[0]
#@value p3: includes={'', 'abc'}
p4 = k in "abc"
#@value p4: types={bool}
p5 = "abc".split(k)[0]
#@value p5: includes={'', 'abc'}
p6 = "abc".find(k)
#@value p6: includes={-1, 0}
p7 = "abc".strip(k)
#@value p7: includes={'bc', 'abc'}
p8 = ["a", "b"].count(k)
#@value p8: types={int}
e = {"a": 1, "b": 2}
p9 = e.pop(k, 0)
#@value p9: includes={0, 1}
p10 = {"a": 1}.get(k, 0)
#@value p10: exactly={0, 1}
d = {}
d.setdefault(k, 1)
p11 = "a" in d
#@value p11: types={bool}
c = Counter()
c[k] += 1
p12 = c["a"]
#@value p12: exactly={0, 1}
f = {"a": 1}
f.update({k: 2})
p13 = f["a"]
#@value p13: includes={1, 2}
s = set()
s.add(k)
p14 = "a" in s
#@value p14: types={bool}
c2 = Counter([k])
p15 = c2["a"]
#@value p15: includes={0, 1}
