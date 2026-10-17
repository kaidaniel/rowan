"""A skipped base `__init__` that gives its receiver away (`getattr(self, "_on_%s" % mode)`:
which method runs is not known) still stores on every way it completes; called through a local
bound to `super().__init__` (argparse's `superinit = super(ArgumentParser, self).__init__`).
Those attributes are there unless code rowan cannot see removed them (`likely`); rowan said
`_items` was only set on some paths and `_groups` was never set (`#@limits budget=0`: every
call is skipped).

Expectations were written from the program text before running the engine."""
#@limits budget=0
import os


class Container:
    def __init__(self, mode):
        self.mode = mode
        self._get_handler()
        self._items = {}
        self._groups = []

    def _get_handler(self):
        return getattr(self, "_on_%s" % self.mode)

    def _on_error(self):
        return 1

    def _on_resolve(self):
        return 2


class Parser(Container):
    def __init__(self, mode="error"):
        superinit = super(Parser, self).__init__
        superinit(mode)
        self.prog = "p"
        print(self._items)  #@ likely _items
        print(self._groups)  #@ likely _groups


p = Parser(os.environ.get("Z634_MODE", "error"))
print(p.prog)
