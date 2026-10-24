"""`"visit_" + kind` where kind may be an instance whose class defines `__radd__`: the name is
whatever that returns (`"reset"`), not a name starting with `visit_`, so getattr may read any
attribute, `reset` among them."""
import os
import sys


class Tag:
    def __radd__(self, other):
        return "reset"


class V:
    def visit_a(self):
        return "a"

    def reset(self):
        return "reset"

    def fallback(self):
        return "fallback"


def pick(kind):
    v = V()
    return getattr(v, "visit_" + kind, v.fallback)()


kind = Tag() if len(sys.argv) > 1 else os.environ.get("Z802_KIND", "a")
r = pick(kind)
#@value r: includes={'reset', 'a'}
