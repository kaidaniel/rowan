# A dict whose insertion order depends on the path, and a set of str (hash order varies per
# run): every use whose result depends on the items' order may see any of them at each place.
import sys
n = len(sys.argv)
e = {}
if n > 2:
    e["a"] = 1
e["b"] = 2
e["a"] = 3
s = {"x", "y"}
xs = []
xs.extend(e)
x0 = xs[0]
#@value x0: exactly={"a", "b"}
ys = []
ys.extend(s)
y0 = ys[0]
#@value y0: exactly={"x", "y"}
p, q = e
#@value p: exactly={"a", "b"}
u, v = s
#@value u: exactly={"x", "y"}
st = [*e]
s0 = st[0]
#@value s0: exactly={"a", "b"}
ss = (*s, "w")
t0 = ss[0]
#@value t0: exactly={"x", "y"}
t2 = ss[2]
#@value t2: exactly={"w"}
def first(*args):
    return args[0]
f0 = first(*e)
#@value f0: exactly={"a", "b"}
g0 = first(*s)
#@value g0: exactly={"x", "y"}
c0 = [k for k in e][0]
#@value c0: exactly={"a", "b"}
d0 = [k for k in s][0]
#@value d0: exactly={"x", "y"}
j = "".join(e)
#@value j: exactly={"ab", "ba"}
z0 = list(zip(e, [1, 2]))[0][0]
#@value z0: exactly={"a", "b"}
m0 = list(dict.fromkeys(s))[0]
#@value m0: exactly={"x", "y"}
l0 = list(e)[0]
#@value l0: exactly={"a", "b"}
n2 = len(xs) + len(ys) + len(st)
#@value n2: exactly={6}
