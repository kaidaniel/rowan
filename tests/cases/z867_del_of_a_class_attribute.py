"""`del Cls.name` removes the class attribute: a later read of it on the class or on an
instance raises AttributeError (a subclass's own is untouched; `getattr` with a default gives
the default); deleting a name the class does not hold itself (a base's, or none) raises.
(Before, `del` on a class changed nothing: the attribute read as before.)

Expectations were written from the program text before running the engine."""
import sys


class Base:
    unit = "m"
    scale = 1


class Sub(Base):
    scale = 2


b = Base()
s = Sub()
del Base.unit
try:
    r = b.unit
except AttributeError:
    r = "attr"
r_seen = r
#@value r_seen: includes={"attr"}  # (Base has no __init__: its instances are open, so any attribute)

try:
    t = Base.unit
except AttributeError:
    t = "attr"
t_seen = t
#@value t_seen: exactly={"attr"}

d = getattr(Base, "unit", -1)
#@value d: exactly={-1}

try:
    del Sub.unit  # (Sub never held it)
    u = "deleted"
except AttributeError:
    u = "attr"
u_seen = u
#@value u_seen: exactly={"attr"}

del Base.scale
sub_scale = s.scale
#@value sub_scale: exactly={2}
try:
    w = b.scale
except AttributeError:
    w = "attr"
w_seen = w
#@value w_seen: includes={"attr"}  # (as r_seen)

if len(sys.argv) > 2:
    del Sub.scale
try:
    x = s.scale
except AttributeError:
    x = "attr"
x_seen = x
#@value x_seen: exactly={2, "attr"}
