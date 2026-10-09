"""An uncalled property on a class never instantiated: a value from an open source or a
default dict; nothing definitely missing.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


class View:
    def __init__(self, src):
        self.src = src

    @property
    def title(self):
        t = self.src.get("title") or {"default": True}
        _ = 0                                #@row t: possible={default} definite_within={default}
        t["default"]                         #@ sound default
        return t
