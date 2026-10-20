# An imported module's `@tagged def f` and `@logged def g` (plain decorators: they run when the
# name is first read, deferred) read on one branch only: after the branches join, the
# decorators have run on every path once the name is read again, so `f.tag` is there and `g`
# is the wrapper on each.
import sys

import tagmod as m

n = len(sys.argv)
if n > 2:
    a = m.f
    b = m.g
x = m.f.tag
#@value x: exactly={"t"}
y = m.g.__name__
#@value y: exactly={"g"}
z = m.g(1)
#@value z: exactly={2}
