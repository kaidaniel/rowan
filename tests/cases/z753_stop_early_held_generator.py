# A generator or iterator held by a name that any / all / `in` stop early on is not used up: a
# later consumer goes on from the item after the deciding one, and sees what the generator did
# up to there only.


def gen(out):
    yield 1
    out["y"] = 2
    yield 2


d = {}
g = gen(d)
r = any(g)
#@value r: exactly={True}
_ = 0
#@row d: present={} definite={} closed
n = next(g, None)
#@value n: exactly={2}
_ = 1
#@row d: present={y} definite={y} closed
it = iter([0, 5, 7, 9])
a = any(it)
b = next(it)
#@value b: exactly={7}
it2 = iter([0, 5, 7, 9])
c = any(x > 6 for x in it2)
#@value c: exactly={True}
d2 = next(it2, None)
#@value d2: exactly={9}
it3 = iter([1, 0, 3])
e = all(it3)
#@value e: exactly={False}
f = len(list(it3))
#@value f: exactly={1}
d3 = {}
g3 = gen(d3)
h = 2 in g3
#@value h: exactly={True}
rest = len(list(g3))
#@value rest: exactly={0}
_ = 2
#@row d3: present={y} definite={y} closed
