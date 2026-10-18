# A generator that yields in branches: an item after a yield that may not have happened is at
# either position. `g(a, b)` gives [], [2], [10] or [2, 10]; `h(a)` gives [10] or [2, 10].
import sys
def g(a, b):
    if a:
        yield 2
    if b:
        yield 10
def h(a):
    if a:
        yield 2
    yield 10
def evens(xs):
    for v in xs:
        if v % 2 == 0:
            yield v
n = len(sys.argv)
ws = list(g(n > 2, n > 3))
k = len(ws)
#@value k: exactly={0, 1, 2}
w0 = ws[0] if ws else -1
#@value w0: exactly={-1, 2, 10}
hs = list(h(n > 2))
h0 = hs[0]
#@value h0: exactly={2, 10}
hl = hs[-1]
#@value hl: includes={10}
#@todo value hl: exactly={10}  (the last item of lists of one of two lengths)
zs = [1, 2 if n > 2 else 3, 7 if n > 2 else 10]
es = list(evens(zs))
e0 = es[0] if es else -1
#@value e0: includes={2, 10}
#@todo value e0: exactly={2, 10}  (zs's items are joined position by position: 3 and 7 come in)
xs = [1, 2, 7]
if n == 1:
    xs = [1, 3, 10]
ys = list(evens(xs))
y0 = ys[0]
#@value y0: includes={2, 10}
#@todo value y0: exactly={2, 10}  (xs's items are joined position by position: 3 and 7 come in)
