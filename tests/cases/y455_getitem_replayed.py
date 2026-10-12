"""A subscript of an instance whose class defines `__getitem__` calls the method; the
instance's item row says nothing about it. A method's summary records `self.factory[name]` as an
item read on its receiver's part; replayed on such an instance (once the tabulation budget is
spent: here 0), it was classified against the empty item row and reported `'To'` definitely
missing (email's `HeaderRegistry`). Likewise an attribute read on an instance whose class
defines `__getattr__`."""
# (summaries are gone since: every call runs at its call site)
class Registry:
    def __init__(self):
        self.registry = {"to": "address"}

    def __getitem__(self, name):
        return self.registry.get(name.lower(), "unstructured")


class Lazy:
    def __getattr__(self, name):
        return name.upper()


class Policy:
    def __init__(self):
        self.header_factory = Registry()
        self.lazy = Lazy()

    def kind(self, name):
        return self.header_factory[name]  #@ sound To

    def flag(self):
        return self.lazy.verbose  #@ sound verbose


p = Policy()
print(p.kind("To"), p.flag())
