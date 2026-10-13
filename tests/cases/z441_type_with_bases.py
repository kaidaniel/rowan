# `type(name, bases, namespace)` makes a class: its bases' methods and attributes, the
# namespace's items, the name given (a name not known is not known); bases that are one of
# several classes make one of several classes.
import os
import sys


class Base:
    kind = "base"

    def describe(self):
        return "I am " + self.kind


class Mixin:
    def extra(self):
        return 42


class Other:
    kind = "other"

    def describe(self):
        return "other"


def make(name, base):
    return type("_" + name, (base, Mixin), {"tag": name})


A = make("A", Base)
a = A()
d = a.describe()  #@value d: exactly={'I am base'}
e = a.extra()  #@value e: exactly={42}
t = a.tag  #@ ok tag
#@value t: exactly={'A'}
n = A.__name__  #@value n: exactly={'_A'}
ok = isinstance(a, Base)  #@value ok: exactly={True}
x = getattr(a, "nothing", None)  #@value x: includes={None}
#@todo value x: exactly={None}  (getattr with a default on a closed instance is not modelled)

U = type(sys.argv[0], (Base,), {})
un = U.__name__  #@value un: unknown
u = U().describe()  #@value u: exactly={'I am base'}

B = Base if os.environ.get("ROWAN_CASE_FLAG") else Other
C = type("C", (B,), {})
cd = C().describe()  #@value cd: exactly={'I am base', 'other'}
