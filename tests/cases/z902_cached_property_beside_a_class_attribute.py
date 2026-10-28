"""A `functools.cached_property` caches its value in the instance only where it ran: where a
subclass's plain class attribute of the name shadows it (here on the paths that did not delete
it), the read gives that and caches nothing, so a second read gives it again. (Before, the
getter's value was cached on every path: the second read was claimed 80 only.)

Expectations were written from the program text before running the engine."""
import functools
import sys


class A:
    @functools.cached_property
    def v(self):
        return 80


class B(A):
    v = 2


if len(sys.argv) > 1:
    del B.v
o = B()
first = o.v
#@value first: exactly={2, 80}
second = o.v
#@value second: exactly={2, 80}
