# An attribute set on the class on one path and on the instance on another: on the third path
# neither has it, and the read raises AttributeError. Found by the fuzzer's per-run seed in CI
# (seed 436001, attrs_436050). python z658.py → 3; z658.py x → 0 (the read raises); z658.py x y → 1.
import sys


class A:
    pass


n = len(sys.argv)
a = A()
r = 0
if n > 2:
    A.u = 1
else:
    if n == 1:
        a.u = 3
try:
    r = a.u
except AttributeError:
    pass
p0 = r
#@value p0: exactly={0, 1, 3}
