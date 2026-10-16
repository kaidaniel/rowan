# A dict whose keys are known but not their order (one was set on some paths first, then on
# every path): each item of an iteration is any of them; sorted, `len` and `in` stay exact.
import sys

n = len(sys.argv)
e = {}
if n % 2 == 0:
    e["m"] = 1
e["z"] = 2
e["m"] = 3
for k in e:
    last = k
last1 = last                                 #@value last1: exactly={"m", "z"}
s = sorted(e)[0]                             #@value s: exactly={"m"}
ks = list(e)
size = len(ks)                               #@value size: exactly={2}
second = ks[1]                               #@value second: exactly={"m", "z"}
