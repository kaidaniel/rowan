"""`len(v)` of a value that is one of two instances runs one `__len__` or the other, not both
in a row: each runs on its own copy of the state and the states join. In a row, B's pop came
after A's store and the read below was taken to fail on every path."""
import sys

G = {}


class A:
    def __len__(self):
        G["x"] = "a"
        return 1


class B:
    def __len__(self):
        G.pop("x", None)
        return 2


v = A() if len(sys.argv) > 1 else B()
n = len(v)
#@value n: exactly={1, 2}
r = G["x"]                                       #@ possibly x
#@value r: exactly={"a"}
