"""A `(a, b) not in d` test that returns narrows `a` on the way on to the literals that some
key of `d` has at its position, also when `a` may be what unknown code returned (the objects
stay): the read `d[(a, b)]` after it cannot miss a literal key (sktime's coercion register:
once nothing havocked the register, this read was a false "never set"). `main` is called
from outside: its parameter is any object.

Expectations were written from the program text before running the engine."""

REG = dict()
REG[("tabular", "transformer")] = len


class Est:
    def get_tag(self, k):
        return "tabular"


def detect(obj):
    if obj == "o":
        return "object"
    return obj.get_tag("kind")


def coerce(obj, to, frm=None):
    if frm is None or len(frm) >= 2:
        frm = detect(obj)
    else:
        frm = frm[0]
    if (frm, to) not in REG:
        return obj
    f = REG[(frm, to)]  #@ sound ('object', 'transformer')
    return f(obj)


def main(a):
    coerce(a, "transformer")
    coerce("o", "transformer", ["tabular"])


coerce("o", "transformer")
coerce(Est(), "transformer")
