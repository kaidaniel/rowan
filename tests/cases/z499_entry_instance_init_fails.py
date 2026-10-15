# Methods nothing calls run on an instance of their class made by `__init__` with its defaults.
# When that `__init__` cannot complete (here: the base class's hook always raises), the entry
# runs of the functions after it (in any order) still run: no instance of Base comes from
# `Base()`, but one of a subclass may be the receiver.


class Base:
    def __init__(self, decls=None):
        self.decls = self._parse(decls)
        self.tag = "base"

    def _parse(self, decls):
        raise NotImplementedError

    def describe(self):
        return self.tag  #@ ok tag


class Impl(Base):
    def _parse(self, decls):
        return list(decls or [])


def later(d):
    x = {"a": 1}
    y = x["a"]  #@ ok a
    #@value y: exactly={1}
    return d["k"]  #@ input k
    #@schema param later.d: always={k} only={k}
