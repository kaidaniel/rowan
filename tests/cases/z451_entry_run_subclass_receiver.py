# A method nothing in the program calls runs with open parameters, its receiver a fresh
# instance of its class. When the class has subclasses, the receiver may be one of theirs: what
# their `__init__`s set may be there (argparse's `_ActionsContainer._registry_get` reads a
# registry only `ArgumentParser.__init__` fills): the read fails on some receivers at most.


class Container:
    def __init__(self):
        self._registries = {}
        self.register("action", "store")

    def register(self, kind, name):
        self._registries.setdefault(kind, {})[name] = 1

    def lookup(self, name):
        regs = self._registries
        return regs["type"].get(name)                          #@ possibly type
        #@todo input type  (Container is never made itself; a subclass's instance has it)


class Parser(Container):
    def __init__(self):
        super().__init__()
        self.register("type", None)


class Plain:
    def __init__(self):
        self.x = 1

    def get(self):
        return self.y                                          #@ missing y
