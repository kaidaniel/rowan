"""An attribute store into None, a number or a str raises AttributeError: `f.extra = 1` with f
a function or None raises where f is None (a module's `try: @dec def f … except: f = None`
then storing on f), and surely on a literal alone.

Expectations were written from the program text before running the engine."""
import os


def h():
    return 1


f = h if os.environ.get("Z925") else None
try:
    f.extra = 1
    r = "stored"
except AttributeError:
    r = "AE"
vr = r                                       #@value vr: exactly={"stored", "AE"}

n = 3 if os.environ.get("Z925N") else "s"
try:
    n.tag = 2
    t = "stored"
except AttributeError:
    t = "AE"
vt = t                                       #@value vt: exactly={"AE"}
