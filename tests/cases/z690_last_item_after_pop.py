# The last item an append put in is known (`xs[-1]` is "s") until something changes the list's
# length: after a `pop` on some paths it is "k" there (a list of unknown length).
import sys
n = len(sys.argv)
xs = []
if n > 2:
    xs.append("k")
xs.append("s")
last0 = xs[-1]
#@value last0: exactly={"s"}
if n > 2:
    if n % 2 == 0:
        xs.pop()
    v = xs[-1]
    #@value v: exactly={"k", "s"}
ys = []
for i in range(n):
    ys.append(i)
ys.append("e")
e = ys[-1]
#@value e: exactly={"e"}
ys.insert(0, "z")
e2 = ys[-1]
#@value e2: includes={"e"}
#@todo value e2: exactly={"e"}  (insert keeps the last item; not tracked)
