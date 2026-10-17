# Dict stores, removals and tests under kx: "m" with three or more arguments, else an unknown
# call's result (it is "k" at run time; for rowan any text). Found by the fuzzer once its dicts
# area got such a key (tests/fuzz.py, DK). Expectations from the program text: with n = 1..5
# arguments (argv[0] included), kx is "k" for n <= 2 and "m" for n >= 3. (`types={bool}`: False or
# True, rowan's bool(?).)
import sys

n = len(sys.argv)
kx = "m" if n > 2 else (sys.argv[0][:0] + "K").lower()
# a del under a key not known may raise (an empty dict: it always does)
e = {}
r = 0
try:
    del e[kx]
except KeyError:
    r = -2
p1 = r
#@value p1: includes={-2}
#@todo value p1: exactly={-2} (an empty dict: the del of any key fails)
# `kk == kx` does not narrow kk to "m": kx may be another text
f = {"k": 1, "m": 2}
for kk in list(f):
    if kk == kx:
        del f[kk]
p2 = "m" in f
#@value p2: types={bool}
p3 = "k" in f
#@value p3: types={bool}
g = {kk: 1 for kk in ("k", "m") if kk != kx}
p4 = "m" in g
#@value p4: types={bool}
# a del of a key a store under a key not known may have made may fail
h = {}
h[kx] = 1
r = 0
try:
    del h["m"]
except KeyError:
    r = -3
p5 = r
#@value p5: exactly={0, -3}
# one dict lacks "k", the other may have it: the read may fail, it does not always
a = {}
b = {}
b[kx] = 1
c = a if n % 2 == 0 else b
try:
    v = c["k"]  #@ possibly "k"
except KeyError:
    v = -4
p6 = v
#@value p6: exactly={1, -4}
# a store under one of two keys keeps what a store under a key not known put there
w = {}
w[sys.argv[0][:0] + "a"] = 0
kl = ["a", "b"][n % 2]
w[kl] = 2
p7 = w.get("a", -1)
#@value p7: includes={0, 2}
# a key removed before may come back under a key not known
x = {"m": 1}
del x["m"]
x[kx] = 2
p8 = "m" in x
#@value p8: types={bool}
