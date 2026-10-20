# An imported module's decorated functions (plain decorators, run when the name is first
# read) changed on one branch only: the other branch never read them. After the join, `f`
# and `g` carry their decorators' tags on every path, and the one branch's change on its
# own: `m.f.tag` is "t" or "z", never missing. `h` is `logged`'s wrapper on every path,
# its `calls` 0 or 5. Rebinding `tagged` after the `def`s changes nothing (the decorator
# ran at the `def`).
import sys

import joinmod as m

n = len(sys.argv)
if n > 2:
    m.f.tag = "z"
    m.g.tag = "y"
    m.h.calls = 5
a = m.f.tag
#@value a: exactly={"t", "z"}
b = m.g.tag
#@value b: exactly={"w", "y"}
c = m.h.calls
#@value c: exactly={0, 5}
d = m.h.__name__
#@value d: exactly={"h"}
e = m.h(3)
#@value e: exactly={6}
p = getattr(m.f, "tag", "-")
#@value p: includes={"t", "z"}
#@todo value p: exactly={"t", "z"}  (decorators run again after the join, joined with the paths they had run on: f as at the `def` joins in, tag maybe missing)
q = getattr(m.g, "tag", "-")
#@value q: includes={"w", "y"}
#@todo value q: exactly={"w", "y"}  (as p)
