"""A class decorator rowan does not model (here one exec made; `@attr.frozen` of a library
alike) may give the class an `__init__` of its own: its instances may have any attributes,
not none of them. (Before, `c.x` was "never set" — a false failure — once nothing unknown
touched the instance.) A regex match's `m[0]` / `m[2]` are its groups' strings.

Expectations were written from the program text before running the engine."""
import re

NS = {}
exec("def frozen(cls):\n    def __init__(self, x):\n        object.__setattr__(self, 'x', x)\n    cls.__init__ = __init__\n    return cls", NS)
frozen = NS["frozen"]


@frozen
class C:
    x: int


c = C(5)
v = c.x                                          #@ input x
#@value v: includes={5}

m = re.match(r"(\w+)-(\d+)", "ab-12")
whole = m[0] if m else None
#@value whole: includes={"ab-12"}
num = m[2] if m else None
#@value num: includes={"12"}
