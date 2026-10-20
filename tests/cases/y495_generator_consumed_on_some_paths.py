"""A generator's body runs at its first consumption. Consumed in one branch only: after the
join the next item is the first on one path and the second on the other, and the body's
first step has run on both paths by the end."""
import sys


def gen(d):
    d["a"] = 1
    yield "x"
    d["b"] = 2
    yield "y"


d = {}
g = gen(d)
if len(sys.argv) > 1:
    first = next(g)                              #@value first: exactly={"x"}
second = next(g)                                 #@value second: exactly={"x","y"}
_ = 0                                            #@row d: present={a,b} closed
#@todo row d: present={a,b} definite={a} closed  (run again where it had not run, its effects joined with the paths it ran on: their consumer may have changed d since, `del d["a"]`)
