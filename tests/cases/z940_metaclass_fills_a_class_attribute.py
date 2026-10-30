"""A metaclass's `__init__` runs when a class statement naming it (or a base naming it) completes:
it may rebind or fill what the class body defined. Here it fills each class's `TABLE` from the
methods the class defines, so a read of `TABLE` sees the registered entries, not the empty dict
the body wrote. Before, rowan ignored the metaclass: `TABLE` was empty, and a read of a key not
known from it held nothing at all.

Expectations were written from the program text before running the engine."""
import sys


class Registry(type):
    def __init__(cls, name, bases, attrs):
        super().__init__(name, bases, attrs)
        table = {}
        for key, value in attrs.items():
            if key.startswith("op_"):
                table[key[3:]] = {"fn": value, "arity": 1}
        cls.TABLE = table


class Ops(metaclass=Registry):
    TABLE = {}

    def op_neg(self, x):
        return -x

    def lookup(self, name):
        try:
            entry = self.TABLE[name]
            # (entry: a dict the metaclass made at L17; rowan ran no metaclass when this said unknown)
        except KeyError:
            return None
        return entry["arity"]


class MoreOps(Ops):
    def op_abs(self, x):
        return abs(x)


n = Ops().lookup(sys.argv[0][:0] + "neg")
#@value n: includes={1}
m = MoreOps().lookup(sys.argv[0][:0] + "abs")
#@value m: includes={1}
