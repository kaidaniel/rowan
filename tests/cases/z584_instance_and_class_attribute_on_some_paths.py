# An attribute an instance has on some paths and its class on others: a read gives either,
# or fails where neither has it.
import sys


class A:
    pass


n = len(sys.argv)
a = A()
if n % 2 == 0:
    A.u = None
if n == 1:
    a.u = "s"
got = getattr(a, "u", -1)                    #@value got: exactly={"s", None, -1}
