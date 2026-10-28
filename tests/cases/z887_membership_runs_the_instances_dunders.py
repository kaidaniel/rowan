"""`key in obj` for an instance of the program's classes asks the instance: its `__contains__`,
else its `__iter__` (or `__getitem__`). Whether the test holds says nothing about keys the
instance itself stores as a dict would: a true test is no reason to drop the branch. Before,
the test narrowed the instance as if it were a dict with no keys, so the branch where it holds
was taken for unreachable: with only `__iter__`, and with a `__contains__` whose receiver may be
an instance of a subclass too (a method's `self` for any call, an instance chosen by a test).
A cache class of a library kept its `if key in self:` branch out of every hover.

Expectations were written from the program text before running the engine."""
import sys


class Bag:
    def __init__(self):
        self.items = ["a", "b"]

    def __iter__(self):
        return iter(self.items)


bag = Bag()
if "a" in bag:
    r = "yes"
else:
    r = "no"
seen_r = r
#@value seen_r: includes={"yes"}  # (iteration decides it: "a" is the first item)


class Store:
    def __init__(self):
        self.__items = {}

    def __getitem__(self, key):
        return self.__items[key]

    def __setitem__(self, key, value):
        self.__items[key] = value

    def __contains__(self, key):
        return key in self.__items

    def take(self, key, default=None):
        if key in self:
            found = self[key]
            #@general found: unknown  # (whatever was stored under the key)
        else:
            found = default
        return found


class Ordered(Store):
    def __init__(self):
        Store.__init__(self)
        self.order = []

    def __setitem__(self, key, value):
        Store.__setitem__(self, key, value)
        self.order.append(key)


box = Store() if len(sys.argv) > 5 else Ordered()
name = sys.argv[0]
box[name] = 1
if name in box:
    got = box[name]
else:
    got = "none"
seen_got = got
#@value seen_got: includes={1}  # (the key was just stored, in either class's instance)

taken = box.take(name)
#@value taken: includes={1}

empty = Ordered()
if name in empty:
    e = "in"
else:
    e = "out"
seen_e = e
#@value seen_e: includes={"out"}
