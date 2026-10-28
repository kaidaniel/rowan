"""`if x in (3, "a", None):` where x may be a parameter's value (anything) beside literals the
function itself gave it: in the branch x is any of the tuple's items, not only those among its
own literals. (Before, `if x is None: x = 4 if c else x` then `x in (3, "a", None)` left only
`None` in the branch for any call: the parameter's own 3 or "a" were lost.)

Expectations were written from the program text before running the engine."""
import sys

c = len(sys.argv) > 2


def pick(x):
    if x is None:
        x = 4 if c else x
    if x in (3, "a", None):
        seen = x
        #@general seen: exactly={3, "a", None}
        return seen
    return 0


def eq(x):
    if c:
        x = 4
    if x == 3:
        same = x
        #@general same: includes={3}  # (not exactly: for any call x may be an object whose `__eq__` says so)
        return same
    return 0


pick(3)
pick("a")
pick(None)
eq(3)
