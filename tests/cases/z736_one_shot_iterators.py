# map, filter, zip, enumerate and reversed give one-shot iterators: a second pass over one
# sees nothing. An iterator advanced on one branch only is still at its start on the other.
import sys

n = len(sys.argv)
m = map(str, [1, 2, 3])
b = len(list(m))
#@value b: exactly={3}
c = len(list(m))
#@value c: exactly={0}
z = zip([1, 2], [3, 4])
d = list(z)
e = len(list(z))
#@value e: exactly={0}
f = filter(None, [0, 1, 2])
g = sum(f)
#@value g: exactly={3}
h = sum(f)
#@value h: exactly={0}
en = enumerate("ab")
for i, x in en:
    pass
k = len(list(en))
#@value k: exactly={0}
r = reversed([1, 2])
r1 = list(r)
r2 = len(list(r))
#@value r2: exactly={0}
m2 = map(abs, [-1, -2])
total = 0
for v in m2:
    total += v
for v in m2:
    total += v
tt = total
#@value tt: exactly={3}
nx = next(m2, "-")
#@value nx: exactly={"-"}
xs = iter([1, 2, 3])
if n > 2:
    a = next(xs)
rest = len(list(xs))
#@value rest: exactly={2, 3}
# (where xs stands differs by path: as many items left as on each)
ys = iter([1, 2, 3])
mm = map(str, ys)
if n > 2:
    cc = list(mm)
left = len(list(mm))
#@value left: exactly={0, 3}
# (as rest)
ky = len(list(ys))
#@value ky: exactly={0}
# (as rest: mm used up on one path, ys at its end on both)
