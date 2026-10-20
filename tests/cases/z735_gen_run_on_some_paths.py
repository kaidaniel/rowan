# A generator consumed on one branch only, the consumer then changing what the generator
# changed: consumed again after the join, it runs on the paths it had not run on, and the
# paths it ran on keep what their consumer did since (`d["k"]` is 5 there, not 1 again).
import sys

d = {}
log = []


def g():
    d["k"] = 1
    yield 1


def h():
    log.append("a")
    yield 1
    log.append("b")


n = len(sys.argv)
it = g()
it2 = h()
if n > 2:
    for x in it:
        pass
    d["k"] = 5
    first = next(it2)
    log.append("c")
y = list(it)
t = len(y)
#@value t: includes={0, 1}
#@todo value t: exactly={0, 1}  (where it is in its items differs by path: not known after the join)
z = d["k"]
#@value z: exactly={1, 5}
rest = list(it2)
r = len(rest)
#@value r: includes={0, 1}
#@todo value r: exactly={0, 1}  (as t)
w = len(log)
#@value w: includes={2, 3}
#@todo value w: exactly={2, 3}  (as t: how far h ran is not known, log's length neither)
last = log[-1]
#@value last: includes={"b"}
#@todo value last: exactly={"b"}  (as w)
