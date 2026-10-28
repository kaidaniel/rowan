"""A method of a plain class in a module that imports numpy may run on an instance of any of
its subclasses, as anywhere else: `self.check(x)` in the base's method calls the subclass's
`check` for any call. Before, a module importing numpy was treated as a torch module, whose
methods take `self` as the class's own instance only: the base's `check` returns None, so
`not None` was all `run` could give and the subclasses' results never reached its hover.

Expectations were written from the program text before running the engine."""
import numpy as np


class Rule:
    def __init__(self, **kwargs):
        self.label = kwargs.get("label")

    def check(self, values):
        """Overridden by each rule."""

    def run(self, values):
        ok = self.check(values)
        #@general ok: includes={True, False, None}
        return ok


class Positive(Rule):
    def check(self, values):
        return bool(np.all(np.asarray(values) > 0))


class Small(Rule):
    def __init__(self, limit: float = 10.0, **kwargs):
        super().__init__(**kwargs)
        self.limit = limit

    def check(self, values):
        return bool(np.all(np.asarray(values) < self.limit))


first = Positive().run([1, 2])
second = Small(limit=1.0).run([3])
